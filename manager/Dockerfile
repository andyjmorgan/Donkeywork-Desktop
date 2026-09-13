FROM node:22-alpine AS ui
WORKDIR /src/console-ui
COPY console-ui/package*.json ./
RUN npm ci
COPY console-ui/ ./
RUN npx tsc --noEmit && npx vite build --config vite.manager.config.ts
FROM golang:1.26-alpine AS backend
WORKDIR /src
COPY manager/go.mod manager/go.sum ./
RUN go mod download
COPY manager/ ./
RUN CGO_ENABLED=0 go build -trimpath -o /manager ./cmd/manager
COPY packaging/agent/ /agent-package/
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -trimpath -o /agent-package/dwdesktop-agent-amd64 ./cmd/device && CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build -trimpath -o /agent-package/dwdesktop-agent-arm64 ./cmd/device
RUN cd /agent-package && chmod 755 install.sh && sha256sum dwdesktop-agent-amd64 dwdesktop-agent-arm64 install.sh dwdesktop-agent.service > SHA256SUMS && tar -czf /dwdesktop-agent-0.1.0-preview6.tar.gz dwdesktop-agent-amd64 dwdesktop-agent-arm64 install.sh dwdesktop-agent.service SHA256SUMS
FROM alpine:3.23
RUN apk add --no-cache ca-certificates && adduser -D -u 10001 manager
COPY --from=backend /manager /usr/local/bin/manager
COPY --from=ui /src/console-ui/dist-manager /app/web
COPY --from=backend /dwdesktop-agent-0.1.0-preview6.tar.gz /app/web/downloads/dwdesktop-agent-0.1.0-preview6.tar.gz
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/manager"]
