import Config

config :logger, :console,
  format: "$time [$level] $metadata| $message\n",
  metadata: [:error_code, :module]
