defmodule Fub.Source.Source do
  @callback get_launch_info(state :: map) :: map
  @callback get_content(state :: map) :: Enumerable.t()
  @callback handle_result(
              state :: map,
              selection :: String.t(),
              query :: String.t(),
              key :: String.t()
            ) :: {:continue, map} | {:exit, String.t()}
  @callback get_preview_command(state :: map) :: {:continue, map} | {:exit, String.t()}
end
