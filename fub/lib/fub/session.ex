defmodule Fub.Session do
  require Logger

  def new(client_socket) do
    # for x <- 0..999_999_0 do
    :ok = :gen_tcp.send(client_socket, "eyeoo\n")
    # end

    :ok = :gen_tcp.send(client_socket, "eit's a hard life\n")
    :ok = :gen_tcp.send(client_socket, "eyeoo yaya ddd\n")
    loop(client_socket)
  end

  defp loop(socket) do
    :ok = :gen_tcp.send(socket, "w\n")
    Logger.info("Waiting for response")
    {:ok, response} = :gen_tcp.recv(socket, 0)
    [code, content] = String.split(response, ":", parts: 2)
    Logger.info("Killing client")
    :ok = :gen_tcp.send(socket, "x#{content}\n")
  end
end
