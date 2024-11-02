import socket
import subprocess
import sys
import json


def open_finder(args):
    sys.stderr.write("DEBUG: " + str(args) + "\n")
    sys.stderr.flush()
    command = [
        "fzf",
        "--query",
        args["query"],
        "--expect",
        ",".join(args["key_bindings"]),
    ]
    return subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)


def main():
    finder_ui = None
    client = socket.socket(socket.AF_UNIX)
    # client.settimeout(1)
    client.connect("/tmp/fuba.socket")
    reader = client.makefile("r")
    client.sendall(json.dumps({"tag": "list-files"}).encode() + b"\n")

    should_respond = False
    while True:
        if finder_ui is not None:
            finder_ui.poll()

            if finder_ui.returncode is not None:
                if finder_ui.returncode > 128:
                    return
                output_lines = finder_ui.stdout.read().decode().split("\n")
                client.sendall(
                    json.dumps(
                        {
                            "tag": "result",
                            "output": output_lines[1],
                            "code": finder_ui.returncode,
                            "key": output_lines[0],
                        }
                    ).encode()
                    + b"\n"
                )
                # f"{finder_ui.returncode}:{output}\n".encode())
                should_respond = False
                finder_ui.wait()
                finder_ui = None

        if not should_respond:
            # print("waiting for response")
            content = reader.readline().strip()
            # print("got response")
            # print(content)
            # sys.stderr.write(f"command: {command}\n")
            match content[0]:
                case "x":  # "exit":
                    sys.stdout.write(content[1:])
                    return
                case "w":  # "wait-for-response":
                    should_respond = True
                case "e":  # case "entry":
                    # print(f"entry: {content[1:]}")
                    finder_ui.stdin.write((content[1:] + "\n").encode())
                    finder_ui.stdin.flush()
                case "o":  # "open-finder":
                    # sys.stderr.write("open-finder\n")
                    # sys.stderr.flush()
                    payload = json.loads(content[1:])
                    finder_ui = open_finder(payload)


if __name__ == "__main__":
    main()
