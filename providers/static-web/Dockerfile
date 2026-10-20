# Optional future container build; not used by the local test.
FROM golang:1.27-alpine AS build
WORKDIR /source
COPY go.mod main.go ./
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/static-web-provider .

FROM scratch
COPY --from=build /out/static-web-provider /static-web-provider
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/static-web-provider"]
