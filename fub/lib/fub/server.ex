defmodule Fub.Server do
  use GenServer
  require Logger

  def start_link([]) do
    GenServer.start_link(__MODULE__, nil, name: __MODULE__)
  end

  @impl true
  def init(nil) do
    socket_name = "/tmp/fuba.socket"
    File.rm(socket_name)
    opts = [:binary, ifaddr: {:local, socket_name}, packet: :line, active: false]

    {:ok, socket} =
      :gen_tcp.listen(0, opts)

    send(self(), :start_loop)
    {:ok, %{socket: socket}}
  end

  def loop(%{socket: socket} = state) do
    Logger.info("Waiting for connection")
    {:ok, listener} = :gen_tcp.accept(socket)
    Logger.info("Accepted connection")
    # {:ok, response} = :gen_tcp.recv(listener, 0)
    # Logger.info("Received: #{inspect(response)}")
    :ok = :gen_tcp.send(listener, "entry:yeoo\n")
    :ok = :gen_tcp.send(listener, "entry:it's a hard life\n")
    :ok = :gen_tcp.send(listener, "entry:yeoo yaya ddd\n")
    :ok = :gen_tcp.send(listener, "wait-for-response:\n")
    {:ok, response} = :gen_tcp.recv(listener, 0)
    [code, content] = String.split(response, ":", parts: 2)
    :ok = :gen_tcp.send(listener, "exit:#{content}\n")
    # {:ok, response} = :gen_tcp.recv(listener, 0)
    # Logger.info("Received: #{inspect(response)}")
    # :ok = :gen_tcp.send(listener, "Ye Boy!\n")

    # new_state =
    #   case message do
    #     "test" ->
    #       :ok = :gen_tcp.send(socket, "yeo")
    #   end

    loop(state)
  end

  @impl true
  def handle_info(:start_loop, state) do
    Logger.info("Looping")
    {:noreply, loop(state)}
  end
end
