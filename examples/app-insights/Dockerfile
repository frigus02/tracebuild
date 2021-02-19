FROM otel/opentelemetry-collector-contrib
COPY config.yaml /etc/otel/config.yaml
EXPOSE 4317 13133
ENTRYPOINT ["/otelcontribcol"]
CMD ["--config", "/etc/otel/config.yaml"]
