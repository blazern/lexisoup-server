#!/usr/bin/env python3
from __future__ import annotations
import argparse
import os
import shutil
import subprocess
import json
import time
from pathlib import Path
import sqlite3
import tempfile
import requests
from tqdm import tqdm


def run(cmd: list[str], *, cwd: Path | None = None) -> None:
    """Pretty wrapper around subprocess.check_call."""
    print(f"> {' '.join(cmd)}")
    subprocess.check_call(cmd, cwd=cwd)


def main() -> None:
    home: str = os.getenv("HOME")
    parser = argparse.ArgumentParser(
        prog="deploy.py",
        description="Fresh-clone repo, "
            "drop release binary in place, "
            "write .env, "
            "download and migrate PanLex DB, "
            "creates suggestions table in the PanLex DB, "
            "restart compose",
    )
    parser.add_argument(
        "--binary-path",
        required=True,
        type=Path,
        help="Absolute path to the compiled backend binary on the server",
    )
    parser.add_argument(
        "--repo-url",
        required=True,
        help="URL of the repository to clone",
    )
    parser.add_argument(
        "--api-key-chatgpt",
        required=True,
        help="ChatGPT API key to insert into .env",
    )
    parser.add_argument(
        "--graphql-parent-path",
        required=True,
        help="Parent path prefix for GraphQL endpoints (e.g. /langample/)",
    )
    parser.add_argument(
        "--panlex-sqlite-db-url",
        required=True,
        help="URL to download panlex.sqlite from (only if missing)",
    )
    parser.add_argument(
        "--panlex-sqlite-db-path",
        required=True,
        help="Absolute path to panlex.sqlite on the server (e.g. ~/panlex.sqlite)",
    )
    parser.add_argument(
        "--deploy-dir",
        type=Path,
        default=Path(f"{home}/langample"),
        help="Root directory for the checkout on the server",
    )
    args = parser.parse_args()

    binary_src: Path = args.binary_path.resolve()
    api_key_chatgpt: str = args.api_key_chatgpt
    graphql_parent_path: str = args.graphql_parent_path
    panlex_sqlite_db_path: Path = Path(args.panlex_sqlite_db_path).expanduser().resolve()
    panlex_sqlite_db_url: str = args.panlex_sqlite_db_url
    deploy_dir: Path = args.deploy_dir
    repo_url: str = args.repo_url

    if not binary_src.is_file():
        raise SystemExit(f"{binary_src} is not a file")

    bin_file_name = "backend-bin"
    compose_dir = deploy_dir / "docker"
    bin_dest = compose_dir / "langample" / bin_file_name
    env_file = compose_dir / ".env"
    panlex_sqlite_db_migration_script_path = deploy_dir / "scripts" / "panlex_sqlite_init_suggestions.sql"
    sqlite_spellfix_prebuilt_path = deploy_dir / "prebuilt" / "linux-x86_64" / "spellfix.so"

    print(f"> Cleaning {deploy_dir}")
    shutil.rmtree(deploy_dir, ignore_errors=True)

    print("> Cloning repo")
    run(["git", "clone", "--depth", "1", repo_url, str(deploy_dir)])

    if not panlex_sqlite_db_migration_script_path.is_file():
        raise SystemExit(f"{panlex_sqlite_db_migration_script_path} is not a file")
    if not sqlite_spellfix_prebuilt_path.is_file():
        raise SystemExit(f"{sqlite_spellfix_prebuilt_path} is not a file")

    print("> Moving release binary")
    bin_dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(binary_src), str(bin_dest))

    print(f"> Generating .env {env_file}")
    compose_dir.mkdir(parents=True, exist_ok=True)
    env_file.write_text(
        f"HOST_PATH_BIN={bin_file_name}\n"
        f"API_KEY_CHATGPT={api_key_chatgpt}\n"
        f"GRAPHQL_PARENT_PATH={graphql_parent_path}\n"
        f"PANLEX_SQLITE_DB_PATH={panlex_sqlite_db_path}\n"
        f"SQLITE_SPELLFIX_PREBUILT_PATH={sqlite_spellfix_prebuilt_path}\n",
        encoding="utf-8",
    )

    print("> Ensuring PanLex DB exists")
    ensure_panlex_db(panlex_sqlite_db_url, panlex_sqlite_db_path)

    print("> Migrating PanLex DB")
    migrate_panlex_db(panlex_sqlite_db_path, panlex_sqlite_db_migration_script_path, sqlite_spellfix_prebuilt_path)

    print("> Restarting compose")
    run(
        [
            "docker",
            "compose",
            "up",
            "-d",
            "--build",
            "--force-recreate",
            "--remove-orphans",
        ],
        cwd=compose_dir,
    )

    print("> Checking containers' health")
    assert_containers_healthy(compose_dir)

    print("Deploy finished")


def ensure_panlex_db(url: str, db_path: Path) -> None:
    db_path.parent.mkdir(parents=True, exist_ok=True)

    if db_path.exists():
        print(f"> PanLex DB already exists, skipping downloading")
        return
    
    with tempfile.NamedTemporaryFile(
        dir=db_path.parent,
        prefix=db_path.name + ".tmp",
        delete=False,
    ) as tmp:
        tmp_path = Path(tmp.name)

    try:
        print(f"> Downloading {url} -> {tmp_path}")
        r = requests.get(url, stream=True, timeout=3600)
        r.raise_for_status()

        total = int(r.headers.get("Content-Length", 0))
        chunk_size = 1024 * 1024

        with open(tmp_path, "wb") as f, tqdm(
            total=total,
            unit="B",
            unit_scale=True,
            unit_divisor=1024,
            desc="PanLex DB",
        ) as bar:
            for chunk in r.iter_content(chunk_size=chunk_size):
                if chunk:
                    f.write(chunk)
                    bar.update(len(chunk))
        os.replace(tmp_path, db_path)
        print(f"> Downloaded to {db_path}")
    finally:
        if tmp_path.exists():
            tmp_path.unlink()


def migrate_panlex_db(
        panlex_sqlite_db_path: Path,
        panlex_sqlite_db_migration_script_path: Path,
        spellfix_so_path: Path,
) -> None:
    conn = sqlite3.connect(str(panlex_sqlite_db_path))
    try:
        conn.enable_load_extension(True)
        conn.execute("SELECT load_extension(?)", (str(spellfix_so_path),))

        if table_exists(conn, "spell"):
            print("'spell' table already exists, no migration will be run.")
            return

        sql_text = panlex_sqlite_db_migration_script_path.read_text(encoding="utf-8")

        print(f"Applying migration from: {panlex_sqlite_db_migration_script_path}")
        conn.executescript(sql_text)
        conn.commit()

        if table_exists(conn, "spell"):
            print("Migration applied and 'spell' exists now.")
        else:
            raise SystemExit(
                "Migration script ran, but 'spell' still does not exist. "
                "Check the SQL file."
            )
    except sqlite3.Error as e:
        try:
            conn.rollback()
        except sqlite3.Error:
            pass
        raise SystemExit(f"SQLite error: {e}") from e
    finally:
        conn.close()


def assert_containers_healthy(
    compose_dir: Path,
    timeout: int = 60,
    stable_seconds: int = 15,
) -> None:
    """
    Waits until every container is continuously healthy.
    """
    deadline = time.time() + timeout
    stable_since: float | None = None

    while time.time() < deadline:
        ps = subprocess.run(
            ["docker", "compose", "ps", "--format", "json"],
            cwd=compose_dir,
            capture_output=True,
            text=True,
            check=True,
        )

        services_json = [
            json.loads(line) for line in ps.stdout.splitlines() if line.strip()
        ]

        states = {
            s["Name"]: (s["State"], s.get("Health", ""))
            for s in services_json
        }

        exited_or_restart = [
            n for n, (st, _) in states.items() if st in ("exited", "restarting")
        ]
        unhealthy = [
            n for n, (_, health) in states.items() if health == "unhealthy"
        ]

        if exited_or_restart or unhealthy:
            raise RuntimeError(
                f"Faulty containers – exited/restarting: {exited_or_restart}, "
                f"unhealthy: {unhealthy}"
            )

        all_ok = all(
            st == "running" and (health in ("", "healthy"))
            for st, health in states.values()
        )

        if all_ok:
            if stable_since is None:
                stable_since = time.time()
            elif time.time() - stable_since >= stable_seconds:
                print(
                    f"✔ All containers running and healthy "
                    f"(stable ≥ {stable_seconds}s)"
                )
                return
        else:
            stable_since = None

        time.sleep(2)

    raise TimeoutError(
        f"Containers not healthy after {timeout}s: {states}"
    )


def table_exists(conn: sqlite3.Connection, name: str) -> bool:
    row = conn.execute(
        "SELECT 1 FROM sqlite_master WHERE name=? AND type IN ('table','view') LIMIT 1",
        (name,),
    ).fetchone()
    return row is not None


if __name__ == "__main__":
    main()
