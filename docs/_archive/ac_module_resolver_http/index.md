# handl_module_resolver_http

# handl-module-resolver-http

HTTP-based [`ModuleResolver`] implementation.

Fetches documents over HTTP(S) using `ureq`. Module paths are URLs:

```text
import('https://raw.githubusercontent.com/org/repo/main/schemas/base.yml')
```

Relative paths are resolved against the base URL (the URL of the
importing document).

## Modules

### [`handl_module_resolver_http`](handl_module_resolver_http.md)

*2 modules*

### [`http_resolver`](http_resolver.md)

*1 struct*

### [`http_resolver::HttpResolver`](http_resolver/HttpResolver.md)

*1 function*

### [`url`](url.md)

*1 function*

