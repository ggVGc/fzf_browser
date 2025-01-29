defmodule Fub.Server do
  use GenServer
  require Logger

  @socket_name if(Mix.env() == :prod, do: "socket", else: "socket_dev")

  def start_link([]) do
    GenServer.start_link(__MODULE__, nil, name: __MODULE__)
  end

  @impl true
  def init(nil) do
    cache_home = System.get_env("XDG_RUNTIME_DIR")


    if is_binary(cache_home) do
      socket_dir = cache_home <> "/fzf_browser/"
      socket_path = socket_dir <> @socket_name

      if not File.dir?(socket_dir) do
        File.mkdir!(socket_dir)
      else
        File.rm(socket_path)
      end

      opts = [:binary, ifaddr: {:local, socket_path}, packet: :line, active: false]
      {:ok, socket} = :gen_tcp.listen(0, opts)
      send(self(), :start_loop)
      {:ok, %{socket: socket}}
    end
  end

  def loop(%{socket: socket} = state) do
    Logger.debug("Waiting for connection")
    {:ok, client_socket} = :gen_tcp.accept(socket)
    Logger.debug("Accepted connection")

    {:ok, session_pid} =
      DynamicSupervisor.start_child(Fub.SessionSupervisor, {Fub.Session, [client_socket]})

    :gen_tcp.controlling_process(client_socket, session_pid)
    loop(state)
  end

  @impl true
  def handle_info(:start_loop, state) do
    Logger.debug("Listening for connections")
    {:noreply, loop(state)}
  end
end
