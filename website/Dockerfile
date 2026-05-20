# Build context: REPO ROOT (not website/).
# docker build -f website/Dockerfile .
# DO App Platform: source_dir: / (root) in .do/app.yaml

# Stage 1: Build the static site
FROM oven/bun:1.2.21-alpine AS builder
WORKDIR /app

# Copy website source + grammar source (needed by sync-grammar prebuild step)
COPY website/ ./
COPY tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json /tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json

RUN bun install --frozen-lockfile
RUN bun run generate

# Stage 2: Serve with nginx (no bun in final image)
FROM nginx:1.27-alpine AS runtime
COPY --from=builder /app/.output/public /usr/share/nginx/html
# SPA-style routing: try files, fall back to index.html for client-side routes
RUN printf 'server {\n  listen 80;\n  root /usr/share/nginx/html;\n  location / {\n    try_files $uri $uri/ /index.html;\n  }\n}\n' > /etc/nginx/conf.d/default.conf
EXPOSE 80
