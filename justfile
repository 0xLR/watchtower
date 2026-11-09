set windows-shell := ["cmd.exe", "/c"]

dev:
    pnpm dev

build:
    pnpm build

test:
    pnpm test

lint:
    pnpm lint

sidecar:
    pnpm sidecar


package:
    pnpm package

smoke:
    pnpm test:smoke
