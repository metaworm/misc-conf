
# misc-conf

Nom parser for nginx/apache configuration

## Features

- Uniform AST for different config formats [`ast::Directive`]
- Resolve included configuration recursively [`ast::DirectiveTrait::resolve_include`]
- Query nodes by specific path [`ast::Directive::query`]
- Zero-copy string by using `Directive<S, Literal>` [`lexer::Literal`]
- Support embed lua configuration for nginx

## Usage

```rust
fn main() -> anyhow::Result<()> {
    use misc_conf::apache::Apache;
    use misc_conf::ast::*;
    use misc_conf::nginx::Nginx;

    let args = std::env::args().collect::<Vec<_>>();
    let f = args[1].as_str();
    println!("{f}");
    let data = std::fs::read(f)?;
    // for nginx configuration
    if let Ok(res) = Directive::<Nginx>::parse(&data) {
        println!("{res:#?}");
    }
    // for apache configuration
    if let Ok(res) = Directive::<Apache>::parse(&data) {
        println!("{res:#?}");
    }

    Ok(())
}
```

## Ngnix example

For such ngnix configuration

    http {
        server {
            listen 80 ssl default_server;

            proxy_set_header Host $host:$server_port;
            proxy_set_header X-Forwarded-for $remote_addr;
            proxy_set_header X-Real-IP $remote_addr;

            proxy_set_header X-Request-Id $request_id;
            proxy_set_header X-Forwarded-Proto $scheme;

            # if ($host ~* ^www\.(.*)$) {
            #     set $host_wo_www $1;
            #     rewrite / https://${host_wo_www}$request_uri permanent;
            # }

            location / {
               gzip on;
               proxy_pass http://localhost:10001;
            }
        }
    }

you will get AST like this

    Directive {
        name: "http",
        args: [],
        children: [
            Directive {
                name: "server",
                args: [],
                children: [
                    Directive {
                        name: "listen",
                        args: [
                            "80",
                            "ssl",
                            "default_server",
                        ],
                    },
                    Directive {
                        name: "proxy_set_header",
                        args: [
                            "Host",
                            "$host:$server_port",
                        ],
                    },
                    Directive {
                        name: "proxy_set_header",
                        args: [
                            "X-Forwarded-for",
                            "$remote_addr",
                        ],
                    },
                    Directive {
                        name: "proxy_set_header",
                        args: [
                            "X-Real-IP",
                            "$remote_addr",
                        ],
                    },
                    Directive {
                        name: "proxy_set_header",
                        args: [
                            "X-Request-Id",
                            "$request_id",
                        ],
                    },
                    Directive {
                        name: "proxy_set_header",
                        args: [
                            "X-Forwarded-Proto",
                            "$scheme",
                        ],
                    },
                    Directive {
                        name: "location",
                        args: [
                            "/",
                        ],
                        children: [
                            Directive {
                                name: "gzip",
                                args: [
                                    "on",
                                ],
                            },
                            Directive {
                                name: "proxy_pass",
                                args: [
                                    "http://localhost:10001",
                                ],
                            },
                        ],
                    },
                ],
            },
        ],
    }

## Apache example

For such apache configuration

    <VirtualHost _default_:443>
      SSLEngine on
      ServerName localhost:443
      SSLCertificateFile "${SRVROOT}/conf/ssl/server.crt"
      SSLCertificateKeyFile "${SRVROOT}/conf/ssl/server.key"
      DocumentRoot "${SRVROOT}/htdocs"
    # DocumentRoot access handled globally in httpd.conf
        CustomLog "${SRVROOT}/logs/ssl_request.log" \
              "%t %h %{SSL_PROTOCOL}x %{SSL_CIPHER}x \"%r\" %b"
        <Directory "${SRVROOT}/htdocs">
            Options Indexes Includes FollowSymLinks
            AllowOverride AuthConfig Limit FileInfo
        Require all granted
        </Directory>
    </virtualhost>

you will get AST like this

    Directive {
        name: "VirtualHost",
        args: [
            "_default_:443",
        ],
        children: [
            Directive {
                name: "SSLEngine",
                args: [
                    "on",
                ],
            },
            Directive {
                name: "ServerName",
                args: [
                    "localhost:443",
                ],
            },
            Directive {
                name: "SSLCertificateFile",
                args: [
                    "${SRVROOT}/conf/ssl/server.crt",
                ],
            },
            Directive {
                name: "SSLCertificateKeyFile",
                args: [
                    "${SRVROOT}/conf/ssl/server.key",
                ],
            },
            Directive {
                name: "DocumentRoot",
                args: [
                    "${SRVROOT}/htdocs",
                ],
            },
            Directive {
                name: "CustomLog",
                args: [
                    "${SRVROOT}/logs/ssl_request.log",
                    "\n",
                    "%t %h %{SSL_PROTOCOL}x %{SSL_CIPHER}x \"%r\" %b",
                ],
            },
            Directive {
                name: "Directory",
                args: [
                    "${SRVROOT}/htdocs",
                ],
                children: [
                    Directive {
                        name: "Options",
                        args: [
                            "Indexes",
                            "Includes",
                            "FollowSymLinks",
                        ],
                    },
                    Directive {
                        name: "AllowOverride",
                        args: [
                            "AuthConfig",
                            "Limit",
                            "FileInfo",
                        ],
                    },
                    Directive {
                        name: "Require",
                        args: [
                            "all",
                            "granted",
                        ],
                    },
                ],
            },
        ],
    }
