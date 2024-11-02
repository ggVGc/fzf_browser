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
    {:ok, socket} = :gen_tcp.listen(0, opts)
    send(self(), :start_loop)
    {:ok, %{socket: socket}}
  end

  def loop(%{socket: socket} = state) do
    Logger.debug("Waiting for connection")
    {:ok, client_socket} = :gen_tcp.accept(socket)
    Logger.debug("Accepted connection")
    Task.Supervisor.start_child(Fub.ClientSupervisor, fn -> Fub.Session.new(client_socket) end)
    loop(state)
  end

  @impl true
  def handle_info(:start_loop, state) do
    Logger.debug("Listening for connections")
    {:noreply, loop(state)}
  end
end
