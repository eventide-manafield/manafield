FROM python:3.13-alpine
RUN pip install --no-cache-dir redis==6.4.0
WORKDIR /app
COPY provider.py /app/provider.py
USER 65532:65532
ENTRYPOINT ["python", "/app/provider.py"]
