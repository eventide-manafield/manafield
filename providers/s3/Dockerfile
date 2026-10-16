FROM python:3.13-alpine

# This adapter controls only Resource readiness/registration. The object data
# path stays directly between an authorized Module and its S3-compatible store.
RUN pip install --no-cache-dir boto3==1.40.0

WORKDIR /app
COPY provider.py /app/provider.py
USER 65532:65532
ENTRYPOINT ["python", "/app/provider.py"]
