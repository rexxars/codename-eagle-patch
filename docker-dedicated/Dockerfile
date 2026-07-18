# Codename Eagle 1.50 dedicated server on Wine.
# 32-bit Win32 app -> needs wine32:i386, so the image is linux/amd64 only
# (runs on Apple Silicon via Docker Desktop's Rosetta emulation).
#
# Build context is the repo root so the image can bake in ../game/common +
# ../game/demo (game/full is deliberately excluded from the image):
#   docker build --platform linux/amd64 -f docker-dedicated/Dockerfile -t ce-dedicated:1.50 .
FROM debian:bookworm-slim

RUN dpkg --add-architecture i386 \
	&& apt-get update \
	&& apt-get install -y --no-install-recommends \
		wine \
		wine32:i386 \
		xvfb \
		xauth \
		procps \
	&& rm -rf /var/lib/apt/lists/*

ENV WINEPREFIX=/wineprefix \
	WINEDEBUG=-all

COPY game/common/ /opt/ce/
COPY game/demo/ /opt/ce/
COPY docker-dedicated/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# 24711/udp = game/session traffic (lobby.exe), 4711/udp = GameSpy query port.
EXPOSE 24711/udp 4711/udp

ENTRYPOINT ["/entrypoint.sh"]
