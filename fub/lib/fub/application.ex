defmodule Fub.Application do
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      {Task.Supervisor, name: Fub.TaskSupervisor},
      Fub.Server,
      {DynamicSupervisor, name: Fub.SessionSupervisor},
    ]

    opts = [strategy: :one_for_one, name: Fub.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
