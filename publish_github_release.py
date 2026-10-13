#!/usr/bin/env python3
"""
OpenOrgel GitHub Release Publisher
Publishes release binaries and installer packages directly to GitHub Releases.
Uses credentials from Git Credential Manager or GITHUB_TOKEN environment variable.
"""

import os
import sys
import json
import urllib.request
import urllib.error
import subprocess
from pathlib import Path

REPO_OWNER = "williamemery2010-debug"
REPO_NAME = "OpenOrgel"

def get_github_token():
    # 1. Environment variable
    token = os.getenv("GITHUB_TOKEN")
    if token:
        return token.strip()
        
    # 2. Git Credential Manager
    try:
        proc = subprocess.Popen(
            ["git", "credential", "fill"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        input_data = "protocol=https\nhost=github.com\n"
        out, _ = proc.communicate(input=input_data)
        for line in out.splitlines():
            if line.startswith("password="):
                return line[len("password="):].strip()
    except Exception as e:
        print(f"Warning: Failed to query git credential helper: {e}")
        
    return None

def get_current_tag():
    try:
        tag = subprocess.check_output(["git", "describe", "--tags", "--exact-match"], text=True).strip()
        return tag
    except Exception:
        try:
            return subprocess.check_output(["git", "describe", "--tags", "--abbrev=0"], text=True).strip()
        except Exception:
            return "Prealpha_v2.0.5"

def get_release_notes(tag):
    timeline_path = Path("project_timeline.txt")
    if not timeline_path.exists():
        return f"OpenOrgel release build for {tag}."
        
    content = timeline_path.read_text(encoding="utf-8")
    # Search for tag in timeline
    lines = content.splitlines()
    found = False
    notes = []
    
    for line in lines:
        if tag in line:
            found = True
            notes.append(line)
            continue
        if found:
            if line.startswith("======") or line.startswith("PHASE ") and len(notes) > 5:
                break
            notes.append(line)
            
    if notes:
        return "\n".join(notes).strip()
    return f"OpenOrgel automated release build for {tag}."

def api_request(url, method="GET", data=None, token=None, headers=None):
    req_headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "OpenOrgel-Publisher",
    }
    if token:
        req_headers["Authorization"] = f"Bearer {token}"
    if headers:
        req_headers.update(headers)
        
    body = None
    if data is not None:
        if isinstance(data, (dict, list)):
            body = json.dumps(data).encode("utf-8")
            req_headers["Content-Type"] = "application/json"
        elif isinstance(data, (bytes, bytearray)):
            body = data
        else:
            body = str(data).encode("utf-8")
            
    req = urllib.request.Request(url, data=body, headers=req_headers, method=method)
    try:
        with urllib.request.urlopen(req) as resp:
            content = resp.read()
            if resp.headers.get_content_type() == "application/json":
                return json.loads(content.decode("utf-8"))
            return content
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode("utf-8", errors="replace")
        print(f"HTTP Error {e.code} for {url}: {err_msg}", file=sys.stderr)
        raise

def get_or_create_release(tag, token):
    # Check if release already exists
    url = f"https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/tags/{tag}"
    try:
        rel = api_request(url, token=token)
        print(f"Found existing release for {tag} (ID: {rel['id']})")
        return rel
    except urllib.error.HTTPError as e:
        if e.code != 404:
            raise
            
    # Create new release
    print(f"Creating new GitHub release for {tag}...")
    create_url = f"https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases"
    notes = get_release_notes(tag)
    payload = {
        "tag_name": tag,
        "name": f"OpenOrgel {tag}",
        "body": notes,
        "draft": False,
        "prerelease": True
    }
    rel = api_request(create_url, method="POST", data=payload, token=token)
    print(f"Successfully created release {tag} (ID: {rel['id']})")
    return rel

def upload_asset(release, file_path, token):
    file_path = Path(file_path)
    if not file_path.exists():
        print(f"Asset file not found: {file_path}", file=sys.stderr)
        return
        
    file_name = file_path.name
    upload_url_template = release.get("upload_url", "")
    base_upload_url = upload_url_template.split("{")[0]
    
    # Check if asset already exists in release
    for asset in release.get("assets", []):
        if asset["name"] == file_name:
            print(f"Asset '{file_name}' already exists in release. Deleting old asset (ID {asset['id']})...")
            del_url = f"https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/assets/{asset['id']}"
            api_request(del_url, method="DELETE", token=token)
            break
            
    upload_url = f"{base_upload_url}?name={file_name}"
    print(f"Uploading '{file_name}' ({file_path.stat().st_size:,} bytes)...")
    
    with open(file_path, "rb") as f:
        file_bytes = f.read()
        
    api_request(
        upload_url,
        method="POST",
        data=file_bytes,
        token=token,
        headers={"Content-Type": "application/octet-stream"}
    )
    print(f"Successfully uploaded: {file_name}")

def main():
    token = get_github_token()
    if not token:
        print("ERROR: Could not acquire GitHub token from environment or Git Credential Manager.", file=sys.stderr)
        sys.exit(1)
        
    tag = sys.argv[1] if len(sys.argv) > 1 else get_current_tag()
    print(f"Targeting release tag: {tag}")
    
    # 1. Ensure distribution package is built
    print("Building distribution installer package...")
    from installer.package_release import build_package
    zip_path = build_package()
    
    # 2. Locate standalone executable
    candidate_bins = [
        Path("rust/target/x86_64-pc-windows-gnullvm/release/openorgelsynth.exe"),
        Path("rust/target/release/openorgelsynth.exe"),
        Path(rf"C:\Users\{os.getenv('USERNAME', 'EME0012')}\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe")
    ]
    exe_path = next((p for p in candidate_bins if p.exists()), None)
    
    # 3. Create or fetch release
    release = get_or_create_release(tag, token)
    
    # 4. Upload standalone executable and complete setup zip
    if exe_path:
        upload_asset(release, exe_path, token)
    if zip_path:
        upload_asset(release, zip_path, token)
        
    print("\n=================================================================")
    print(f"Release publishing complete: https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/tag/{tag}")
    print("=================================================================")

if __name__ == "__main__":
    main()
