# Sample Python file for Mantis E2E testing
# Tests: Syntax highlighting, search, line numbers

import sys
import os

class RepositoryExplorer:
    def __init__(self, root_path):
        self.root_path = os.path.abspath(root_path)

    def list_files(self):
        """Recursively list files in the repository root."""
        file_list = []
        for root, dirs, files in os.walk(self.root_path):
            for file in files:
                file_list.append(os.path.join(root, file))
        return file_list

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "."
    explorer = RepositoryExplorer(path)
    print(f"Exploring: {explorer.root_path}")
    files = explorer.list_files()
    print(f"Found {len(files)} files.")
