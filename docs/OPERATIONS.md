# Local operations

## Start and stop

```bash
./scripts/setup.sh
./scripts/up.sh
./scripts/smoke.sh
./scripts/down.sh
```

`setup.sh` generates local-only `.env` credentials and keypairs under ignored `.local/` and `target/` paths. `up.sh` starts services, deploys the Anchor program only to localhost Solana RPC endpoints, and configures the source/destination domains. Never use these scripts with shared or production RPC URLs.

## Ports

| Service | Local port |
|---|---:|
| Web console | 3000 |
| API | 8080 |
| Source Solana RPC / faucet | 8899 / 9900 |
| Destination Solana RPC / faucet | 8900 / 9901 |
| PostgreSQL | 5433 |
| Redis | 6379 |

PostgreSQL and Redis use named Docker volumes. For a destructive local reset, run `docker compose down -v` after stopping services.

## Troubleshooting

- If deployment fails, confirm both local RPC endpoints report healthy and Anchor/Solana CLI versions match `Anchor.toml`.
- If the program ID changed after key generation, rerun `anchor keys sync` and rebuild before deployment.
- If a port is busy, edit the relevant Compose port mapping.
- If a local database volume predates a migration change, remove local volumes and restart; this deletes local data.
- Message APIs stay empty until the not-yet-implemented scanner persists Anchor event observations.
