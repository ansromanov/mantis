# Sample Python file for Mantis E2E testing
# Tests: Syntax highlighting, search, line numbers

import sys
import os
from collections import Counter

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

    def filter_by_extension(self, files, extension):
        """Filter files by a specific extension."""
        return [f for f in files if f.endswith(extension)]

    def get_extension_stats(self, files):
        """Get counts of different file extensions."""
        extensions = [os.path.splitext(f)[1] for f in files if os.path.splitext(f)[1]]
        return Counter(extensions)

    def get_total_size(self, files):
        """Calculate the total size of all listed files in bytes."""
        total_size = 0
        for f in files:
            try:
                total_size += os.path.getsize(f)
            except OSError:
                continue
        return total_size

    def format_size(self, size_in_bytes):
        """Format size in a human-readable format."""
        for unit in ['B', 'KiB', 'MiB', 'GiB']:
            if size_in_bytes < 1024.0:
                return f"{size_in_bytes:.2f} {unit}"
            size_in_bytes /= 1024.0
        return f"{size_in_bytes:.2f} TiB"

    def print_report(self):
        """Print a detailed directory analysis report."""
        print("=" * 40)
        print(f"Directory Analysis Report: {self.root_path}")
        print("=" * 40)
        files = self.list_files()
        print(f"Total files found: {len(files)}")
        
        stats = self.get_extension_stats(files)
        print("\nExtension breakdown:")
        for ext, count in stats.most_common():
            print(f"  {ext}: {count}")

        total_bytes = self.get_total_size(files)
        print(f"\nTotal folder size: {self.format_size(total_bytes)}")
        print("=" * 40)

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "."
    explorer = RepositoryExplorer(path)
    explorer.print_report()
