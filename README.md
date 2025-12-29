# Lexisoup - Infrastructure & Deployment

This repository contains the Lexisoup backend source code, but also the infrastructure and CI/CD setup for deploying the Lexisoup Rust backend to a single Ubuntu VPS:

- An **Ansible** playbook set up an VPS (Docker, nginx, TLS via certbot, PanLex DB download).
- A small **Python deploy script** that drops a pre-built backend binary from Github Releases on the server and restarts the Docker stack.
- A set of **GitHub Actions workflows** that:
  - Build and release the Rust backend binary.
  - Execute the Ansible playbook.
  - Execute the Python deploy script.

The Rust source code itself lives in the `backend/` directory (not documented here in detail).

## High-level deployment flow

1. You push to the `main` branch.
2. GitHub Actions workflow **`build-release`**:
   - Builds the Rust backend and creates a GitHub Release with the built binary.
3. Workflow **`run-ansible`**:
   - SSH-es into the VPS using configured secrets.
   - Runs `ansible/playbook.yaml` to:
     - Install Docker, nginx, certbot, and base packages.
     - Configure nginx as a reverse proxy with TLS via Let’s Encrypt.
     - Expose the backend under `https://<your-domain>/api/…`.
4. Workflow **`deploy-rust`**:
   - Downloads the latest `backend` release asset.
   - Uploads the binary and `scripts/deploy_backend.py` to the VPS.
   - Runs `deploy.py` via SSH, which:
     - Fresh-clones the repo on the server.
     - Copies the backend binary into `docker/langample/` as `backend-bin`.
     - Writes a `.env` file with runtime settings (ChatGPT API key, PanLex path, GraphQL parent path).
     - Download and migrates the PanLex SQLite DB.
     - Runs `docker compose up -d` in `docker/`.
     - Waits until all containers are healthy.

## Requirements & prerequisites to start the server

### 1. VPS (Virtual Private Server)

You need a VPS from any provider (Ramnode, Hetzner, DigitalOcean, etc.) with:

- **OS**: Ubuntu (`apt` and `snap` are assumed to be available).
- Public IP.
- SSH access with a private key (used by GitHub Actions).

### 2. Domain name

You need a domain for the service, e.g.:

- `example.com` or
- `api.example.com`

### 3. DNS configuration

At your domain provider (Cloudflare, Namecheap, etc.), set up at least the `A` DNS record.

Make sure DNS has propagated and that `curl http://your-domain/` reaches the VPS **before** running the Ansible provisioning, otherwise Let’s Encrypt / certbot cannot validate the domain.

### 4. PanLex SQLite database

The backend needs a **PanLex SQLite DB file** that is accessible from the VPS to download.

You can

1. Upload the PanLex DB to somewhere like **AWS S3**.
2. Use an HTTPS URL for downloading. You could put a file into a public bucket/object with restricted access (e.g. limited by IP).

## Github secrets and variables

### Secrets

* **`SERVER_SSH_IP`** - Public IP address of the VPS used for SSH/SCP connections.
* **`SERVER_SSH_USER`** - SSH username for the VPS (e.g. `root`).
* **`SERVER_SSH_KEY`** - Private SSH key GitHub Actions uses to log in to the VPS.
* **`CERTBOT_EMAIL`** - Email address used by Certbot for TLS certificate registration.
* **`SERVER_HOSTNAME`** - Domain name served by nginx and used by Certbot (e.g. `lexisoup.com`).
* **`API_KEY_CHATGPT`** - OpenAI API key injected into the backend container.

### Variables

* **`PANLEX_DB_URL`** - Direct download URL of the PanLex SQLite DB (S3 or other hosting).
* **`PANLEX_SQLITE_DB_PATH`** - Absolute path on the VPS where the PanLex DB will be stored (e.g. `/root/panlex.sqlite`).
