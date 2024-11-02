defmodule Fub.Session do
  require Logger

  @key_bindings ["ctrl-y"]

  def new(client_socket) do
    loop(client_socket)
  end

  defp loop(socket) do
    Logger.debug("Waiting for message")

    case :gen_tcp.recv(socket, 0) do
      {:ok, message} ->
        handle_message(socket, Jason.decode!(message))
        loop(socket)

      {:error, :closed} ->
        Logger.debug("Connection closed")

      {:error, :enotconn} ->
        Logger.debug("Not connected")
    end
  end

  defp handle_message(socket, %{"tag" => "list-files"} = message) do
    Logger.debug("Received message: #{inspect(message)}")

    respond(socket, :open_finder, %{query: "the query", key_bindings: @key_bindings})
    respond(socket, :entry, "neo")
    Process.sleep(1)
    respond(socket, :wait_for_response)
  end

  defp handle_message(socket, %{"tag" => "result"} = message) do
    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        case Map.fetch!(message, "key") do
          "" ->
            respond(socket, :exit, Map.fetch!(message, "output"))
        end
    end
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
