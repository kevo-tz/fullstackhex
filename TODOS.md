# FullStackHex Open Source TODOS

1. Verify if all artifact mentioned in docs folder can be initialized by install script in scripts folder
2. Architecture review for development setup
    a). install script to create folders
        - backend instead of rust-backend, this will include rust crates and python-sidecar
        - frontend
        - migration this folder will be used by sqlx for db migration
    b). only docker-compose.dev.yml to be remain, the rest of docker related file to put in production folder
3. Architecture review for production setup
    a). all production files/artifact to be inside the production folder
    b). Podman Quadlet with systemd will handle all containers
    c). No .env files in production, podman secrets will be used
    d). CI relese action will be used to create deployment arifacts
    e). Deployment artifacts will be pushed to VPS via ssh and rsync
    f). Productio script will be used to create quadlet container with ci release version
    g). If new release fail, roll back to last release, new issue created in github

