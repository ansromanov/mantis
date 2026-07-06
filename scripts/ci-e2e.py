#!/usr/bin/env python3
import os
import pty
import select
import subprocess
import sys
import time

def run_e2e_test():
    print("Starting E2E whole-binary smoke test...")

    # Build the binary first to ensure it's up to date
    print("Building mantis...")
    try:
        subprocess.run(["cargo", "build"], check=True)
    except subprocess.CalledProcessError as e:
        print(f"Failed to build mantis: {e}")
        return False

    binary_path = "./target/debug/mantis"
    test_dir = "./e2e/data"

    if not os.path.exists(binary_path):
        print(f"Binary not found at {binary_path}")
        return False

    # Spawn mantis under a pseudo-terminal (PTY)
    pid, fd = pty.fork()

    if pid == 0:
        # Child process: execute mantis
        try:
            os.execvp(binary_path, [binary_path, test_dir])
        except Exception as e:
            sys.stderr.write(f"Child exec failed: {e}\n")
            sys.stderr.flush()
            sys.exit(127)
    else:
        # Parent process: monitor and interact with mantis
        import fcntl
        import termios
        import struct

        # Set terminal size to 80x24 so TUI renders correctly
        try:
            size_struct = struct.pack("HHHH", 24, 80, 0, 0)
            fcntl.ioctl(fd, termios.TIOCSWINSZ, size_struct)
        except Exception as e:
            print(f"Warning: could not set terminal size: {e}")

        success = False
        output = b""
        start_time = time.time()
        timeout = 5.0  # seconds

        try:
            # Phase 1: Wait for mantis to render and display the files
            print("Waiting for TUI to initialize...")
            while time.time() - start_time < timeout:
                r, _, _ = select.select([fd], [], [], 0.1)
                if fd in r:
                    try:
                        data = os.read(fd, 4096)
                        if not data:
                            break
                        output += data
                        # Look for file names from the test dataset in the TUI screen output
                        if b"rust_sample.rs" in output or b"yaml_sample.yml" in output:
                            print("TUI initialized successfully (detected file tree).")
                            success = True
                            break
                    except OSError:
                        break

            if not success:
                print("Timeout or error waiting for TUI initialization.")
                print(f"Captured output so far: {output.decode('utf-8', errors='ignore')}")
                os.close(fd)
                os.kill(pid, 9)
                return False

            # Phase 2: Send 'q' to quit the application gracefully
            print("Sending 'q' to exit gracefully...")
            os.write(fd, b"q")

            # Phase 3: Wait for process to exit and verify exit code
            exit_success = False
            exit_start = time.time()
            while time.time() - exit_start < 2.0:
                # Wait for child process status change
                try:
                    wpid, status = os.waitpid(pid, os.WNOHANG)
                    if wpid == pid:
                        if os.WIFEXITED(status):
                            exit_code = os.WEXITSTATUS(status)
                            print(f"Mantis exited with code: {exit_code}")
                            if exit_code == 0:
                                exit_success = True
                            break
                        elif os.WIFSIGNALED(status):
                            print(f"Mantis killed by signal: {os.WTERMSIG(status)}")
                            break
                except ChildProcessError:
                    # Child already exited and reaped, or similar
                    exit_success = True
                    break
                time.sleep(0.05)

            os.close(fd)

            if exit_success:
                print("E2E whole-binary smoke test PASSED!")
                return True
            else:
                print("Mantis failed to exit cleanly or exited with non-zero status.")
                try:
                    os.kill(pid, 9)
                    os.waitpid(pid, 0)
                except ChildProcessError:
                    pass
                return False

        except Exception as e:
            print(f"E2E test encountered an exception: {e}")
            try:
                os.close(fd)
                os.kill(pid, 9)
            except Exception:
                pass
            return False

if __name__ == "__main__":
    if not run_e2e_test():
        sys.exit(1)
