# Standard Library — HTTP

Basic HTTP client and server built-in. Not a full framework — enough to make API calls and serve endpoints without any imports.

---

## HTTP Client

All HTTP methods use the `errors` system — handle or propagate.

```
let response = http.get("https://api.example.com/users")
if (response.failed()) {
  log(response.message)
  return
}
let users = response.json()

let response = http.post("https://api.example.com/users", {
  body: { name: "Alice", email: "alice@example.com" },
  headers: { authorization: "Bearer token123" }
})

http.put(url, options)
http.delete(url, options)
http.patch(url, options)
```

---

## HTTP Server

```
let server = http.serve(3000)

server.get("/users", (request: Request) -> Response => {
  let users = loadUsers()
  return response.json(users)
})

server.post("/users", (request: Request) -> Response => {
  let body = request.json()
  let user = createUser(body)
  return response.json(user, status: 201)
})

server.start()
```

---

## Expansion Candidates

- WebSocket support
- Middleware pattern for servers
- Request/response streaming
- File upload handling
- Cookie management
- CORS configuration
- HTTPS/TLS configuration
- Rate limiting helpers
- Request timeout configuration
- Retry logic with backoff
