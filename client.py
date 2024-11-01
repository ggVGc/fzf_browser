import socket
import subprocess
import sys


def open_finder(_query):
    return subprocess.Popen("fzf", stdin=subprocess.PIPE, stdout=subprocess.PIPE)


def main():
    finder_ui = open_finder("")
    client = socket.socket(socket.AF_UNIX)
    client.connect("/tmp/fuba.socket")
    reader = client.makefile("r")

    should_respond = False
    while True:
        finder_ui.poll()
        if finder_ui.returncode is not None:
            output = finder_ui.stdout.read()
            client.sendall(f"{finder_ui.returncode}:{output.decode()}\n".encode())
            should_respond = False
            finder_ui.wait()
            finder_ui = None

        if not should_respond:
            (command, content) = reader.readline().strip().split(":", 1)
            sys.stderr.write(f"command: {command}\n")
            match command:
                case "exit":
                    sys.stdout.write(content)
                    return
                case "wait-for-response":
                    should_respond = True
                case "entry":
                    finder_ui.stdin.write((content + "\n").encode())
                    finder_ui.stdin.flush()
                case "open-finder":
                    finder_ui = open_finder("")


if __name__ == "__main__":
    main()
