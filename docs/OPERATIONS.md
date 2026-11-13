# Local operations

## Start and stop

```bash
./scripts/setup.sh
./scripts/up.sh
./scripts/smoke.sh
./scripts/down.sh
```

`setup.sh` creates `.env` only when absent. Compose health checks gate service startup. `up.sh` deploys contracts after the node containers start. Local deployment addresses are written to ignored `contracts/deployments/local.json`.

## Ports

| Service | Local port |
|---|---:|
| Web console | 3000 |
| API | 8080 |
| Source Anvil | 8545 |
| Destination Anvil | 8546 |
| PostgreSQL | 5433 |
| Redis | 6379 |

PostgreSQL and Redis use named Docker volumes. For a clean reset, stop the stack with `docker compose down -v`.

## Troubleshooting

- If contract deployment fails, confirm both Anvil endpoints respond and Foundry is installed.
- If a port is busy, stop the existing service or edit the corresponding Compose port mapping.
- If a database volume predates a migration change, recreate only local state with `docker compose down -v` then start again.
- API records are in memory and disappear when the API container restarts.

