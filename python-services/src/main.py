from fastapi import FastAPI
from pydantic import BaseModel

app = FastAPI(title="Bare Metal Python Service")

class HelloResponse(BaseModel):
    message: str
    service: str
    status: str

@app.get("/", response_model=HelloResponse)
async def hello():
    return HelloResponse(
        message="Hello from Python!",
        service="python-services",
        status="ok"
    )

@app.get("/health")
async def health():
    return {"status": "healthy"}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
