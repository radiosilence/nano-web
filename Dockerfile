FROM golang:latest as builder
WORKDIR /app
COPY main.go .
COPY go.mod .
COPY go.sum .
RUN CGO_ENABLED=0 GOOS=linux go build -o /serve

FROM alpine:latest
WORKDIR /
COPY --from=builder /serve .
ENV PORT=80
EXPOSE $PORT
CMD ["/serve"]