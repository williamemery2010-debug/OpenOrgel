import subprocess
import os
from datetime import datetime

def run_git_cmd(args):
    try:
        res = subprocess.run(args, capture_output=True, text=True, check=True, encoding="utf-8")
        return res.stdout.strip()
    except Exception as e:
        return f"Error: {e}"

def find_repo_root():
    cur = os.path.dirname(os.path.abspath(__file__))
    while cur and os.path.dirname(cur) != cur:
        if os.path.exists(os.path.join(cur, ".git")):
            return cur
        cur = os.path.dirname(cur)
    return os.path.dirname(os.path.abspath(__file__))

def main():
    root_dir = find_repo_root()
    # Get all tags with their dates and commits
    tags_raw = run_git_cmd(["git", "tag", "-l"])
    tags = [t.strip() for t in tags_raw.split("\n") if t.strip()]
    
    releases_info = []
    for tag in tags:
        commit_hash = run_git_cmd(["git", "rev-list", "-n", "1", tag])
        tag_date = run_git_cmd(["git", "log", "-1", "--format=%ad", "--date=short", tag])
        tag_msg = run_git_cmd(["git", "tag", "-l", "-n9", tag])
        
        # Clean up tag message
        if tag_msg.startswith(tag):
            tag_msg = tag_msg[len(tag):].strip()
        releases_info.append({
            "tag": tag,
            "commit": commit_hash[:7],
            "date": tag_date,
            "message": tag_msg or "No release notes"
        })
    
    # Sort releases by version or date descending
    releases_info.sort(key=lambda x: x["date"], reverse=True)

    # Get commits
    commits_raw = run_git_cmd(["git", "log", "--pretty=format:%h|%ad|%an|%s", "--date=short"])
    commits_list = []
    for line in commits_raw.split("\n"):
        if "|" in line:
            parts = line.split("|", 3)
            if len(parts) == 4:
                commits_list.append(parts)

    # Format the file
    content = []
    content.append("================================================================================")
    content.append("                   OPENORGEL PROJECT HISTORY & RELEASES")
    content.append("================================================================================")
    content.append(f"Updated At     : {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    content.append(f"Total Commits  : {len(commits_list)}")
    content.append(f"Total Releases : {len(releases_info)}")
    content.append("================================================================================\n")

    content.append("🚀 RELEASES / TAGS")
    content.append("--------------------------------------------------------------------------------")
    if not releases_info:
        content.append("No releases found.")
    for rel in releases_info:
        content.append(f"Version : {rel['tag']}")
        content.append(f"Date    : {rel['date']}")
        content.append(f"Commit  : {rel['commit']}")
        content.append(f"Details : {rel['message']}")
        content.append("-" * 80)
    content.append("\n")

    content.append("📜 COMMIT HISTORY (Newest First)")
    content.append("--------------------------------------------------------------------------------")
    format_header = f"{'HASH':<8} | {'DATE':<10} | {'AUTHOR':<20} | {'COMMIT MESSAGE'}"
    content.append(format_header)
    content.append("-" * 80)
    for h, d, a, m in commits_list:
        a_trunc = a[:20]
        content.append(f"{h:<8} | {d:<10} | {a_trunc:<20} | {m}")
    content.append("================================================================================")

    out_content = "\n".join(content)

    targets = [
        os.path.join(root_dir, "commits_and_releases.txt"),
        os.path.join(root_dir, "POC", "HISTORICAL", "commits_and_releases.txt"),
        os.path.join(root_dir, "Open-Orgel Software", "Audio_and_Logs", "commits_and_releases.txt"),
    ]

    for output_file in targets:
        dir_name = os.path.dirname(output_file)
        if os.path.exists(dir_name):
            try:
                with open(output_file, "w", encoding="utf-8") as f:
                    f.write(out_content)
                print(f"Updated {output_file}")
            except Exception as e:
                print(f"Error writing to {output_file}: {e}")

if __name__ == "__main__":
    main()
