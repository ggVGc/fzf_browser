import socket
import subprocess
import sys
import json
import os


def open_fzf(args):
    # sys.stderr.write("DEBUG: " + str(args) + "\n")
    sys.stderr.flush()
    # ansi = []

    command = ["fzf"]

    if "with_ansi_colors" in args and args["with_ansi_colors"]:
        command = command + ["--ansi"]

    command = command + [
        "--print-query",
        "--query",
        args["query"],
        "--expect",
        ",".join(args["key_bindings"]),
    ]
    return subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)


def main():
    fzf = None
    client = socket.socket(socket.AF_UNIX)
    # client.settimeout(1)
    client.connect("/tmp/fuba.socket")

    client.sendall(
        json.dumps(
            {
                "tag": "client_init",
                "start_directory": os.path.abspath(os.getcwd()),
            }
        ).encode()
        + b"\n"
    )

    reader = client.makefile("r")

    read_content = True
    while True:
        if fzf is not None:
            fzf.poll()

            if fzf.returncode is not None:
                if fzf.returncode > 128:
                    return
                output_lines = fzf.stdout.read().decode().split("\n")
                client.sendall(
                    json.dumps(
                        {
                            "tag": "result",
                            "query": output_lines[0],
                            "key": output_lines[1],
                            "selection": output_lines[2],
                            "code": fzf.returncode,
                        }
                    ).encode()
                    + b"\n"
                )
                # f"{fzf.returncode}:{output}\n".encode())
                read_content = True
                fzf.wait()
                fzf = None

        if read_content:
            # print("waiting for response")
            content = reader.readline().strip()
            # print("got response")
            # print(content)
            # sys.stderr.write(f"command: {command}\n")
            match content[0]:
                case "z":  # "end of content":
                    read_content = False
                case "x":  # "exit":
                    sys.stdout.write(content[1:])
                    return
                case "e":  # case "entry":
                    # print(f"entry: {content[1:]}")
                    if fzf is not None:
                        fzf.stdin.write((content[1:] + "\n").encode())
                        fzf.stdin.flush()
                case "o":  # "open-finder":
                    # sys.stderr.write("open-finder\n")
                    # sys.stderr.flush()
                    payload = json.loads(content[1:])
                    fzf = open_fzf(payload)


if __name__ == "__main__":
    main()
