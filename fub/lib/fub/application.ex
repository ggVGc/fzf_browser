defmodule Fub.Application do
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      Fub.Server
    ]

    opts = [strategy: :one_for_one, name: Fub.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
