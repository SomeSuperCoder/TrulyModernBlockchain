import Config

config :networking,
  max_connections: 50,
  discovery_interval: 30_000,
  heartbeat_interval: 10_000,
  bootstrap_nodes: []

import_config "#{config_env()}.exs"
