#!/usr/bin/env python

import socket
import subprocess
import sys
import json
import os


def open_fzf(args):
    # sys.stderr.write("DEBUG: " + str(args) + "\n")
    sys.stderr.flush()
    # ansi = []

    command = ["fzf", "--prompt", args["prompt_prefix"] + ": "]

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
    reading_entries = False
    wait_for_empty = False

    while True:
        if fzf is not None:
            fzf.poll()

            if fzf.returncode is not None:
                # print(f"FZF exited with code {fzf.returncode}")
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
                if reading_entries:
                    wait_for_empty = True
                reading_entries = False
                fzf.wait()
                fzf = None

        if read_content:
            if reading_entries:
                entry = reader.readline()

                if entry == "\n":
                    reading_entries = False
                else:
                    try:
                        fzf.stdin.write((entry).encode())
                        fzf.stdin.flush()
                    except Exception:
                        reading_entries = False
            elif wait_for_empty:
                entry = reader.readline().strip()
                if entry == "":
                    wait_for_empty = False
            else:
                # print("waiting for response")
                content = reader.readline().strip()
                # print("got response")
                # print(content)
                # sys.stderr.write(f"command: {command}\n")
                cmd = content[0]
                match cmd:
                    case "z":  # "end of content":
                        read_content = False
                    case "x":  # "exit":
                        sys.stdout.write(content[1:])
                        return
                    case "e":  # case "begin-entries":
                        reading_entries = True

                    case "o":  # "open-finder":
                        # sys.stderr.write("open-finder\n")
                        # sys.stderr.flush()
                        payload = json.loads(content[1:])
                        fzf = open_fzf(payload)

                    case _:
                        # Escape char
                        if ord(cmd) == 27:
                            pass
                        else:
                            # TODO: Fix bidirectional communication so that this doesn't happen
                            sys.stderr.write(f"Unhandled command string:{cmd}\n")
                            # return


if __name__ == "__main__":
    main()
