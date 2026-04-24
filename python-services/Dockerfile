FROM python:3.14-slim AS builder

WORKDIR /app
COPY pyproject.toml uv.lock* ./
RUN pip install uv && uv pip install --system -e .

COPY src/ ./src/

FROM python:3.14-slim
WORKDIR /app
COPY --from=builder /usr/local/lib/python3.14/site-packages ./site-packages
COPY --from=builder /app/src ./src
COPY --from=builder /app/pyproject.toml .

EXPOSE 8000
CMD ["python", "-m", "uvicorn", "src.main:app", "--host", "0.0.0.0", "--port", "8000", "--loop", "uvloop"]
