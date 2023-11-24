FROM golang:latest
WORKDIR /app
COPY main.go .
COPY go.mod .
COPY go.sum .
RUN CGO_ENABLED=0 GOOS=linux go build -o /serve
