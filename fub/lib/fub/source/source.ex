defmodule Fub.Source.Source do
  @callback get_query_prefix(state :: map) :: String.t()
  @callback get_key_bindings(state :: map) :: map
  @callback get_query(state :: map) :: String.t()
  @callback get_content(state :: map) :: Enumerable.t()
  @callback handle_result(
              state :: map,
              selection :: String.t(),
              query :: String.t(),
              key :: String.t()
            ) :: {:continue, map} | {:exit, String.t()}
end
