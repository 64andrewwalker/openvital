import json
import subprocess

def check_prs():
    res = subprocess.run(["curl", "-s", "-H", "Accept: application/vnd.github+json", "https://api.github.com/repos/punkpeye/openvital/pulls?state=open"], capture_output=True, text=True)
    if not res.stdout:
        print("No output from curl")
        return
    prs = json.loads(res.stdout)
    for pr in prs:
        if isinstance(pr, dict) and pr.get("title", "").startswith("[jules-docs]"):
            print("Found PR:", pr["title"], pr["html_url"])
            return pr
    print("No existing PR found.")

check_prs()
