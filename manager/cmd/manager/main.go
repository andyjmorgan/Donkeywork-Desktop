package main

import (
	"context"
	"crypto"
	"crypto/tls"
	"crypto/x509"
	"flag"
	"log"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"

	manager "donkeywork-desktop/manager"
	"github.com/jackc/pgx/v5/pgxpool"
)

func main() {
	listen := flag.String("listen", "127.0.0.1:8443", "internal bind address")
	origin := flag.String("origin", "https://desktops.donkeywork.dev", "exact operator/enrollment origin")
	tlsCert := flag.String("tls-cert", "", "HTTPS certificate PEM")
	tlsKey := flag.String("tls-key", "", "HTTPS private key PEM")
	caCert := flag.String("device-ca-cert", "", "device signing CA PEM")
	caKey := flag.String("device-ca-key", "", "device signing key PEM")
	pepperFile := flag.String("enrollment-secret-file", "", "persistent file containing at least 32 secret bytes")
	webRoot := flag.String("web-root", "", "built manager frontend directory")
	deviceListen := flag.String("device-listen", "", "separate mTLS listener")
	deviceURL := flag.String("device-url", "", "advertised wss device endpoint")
	mediaListen := flag.String("media-listen", "", "browser WebRTC UDP bind; published port must match")
	mediaIP := flag.String("media-ip", "", "browser-reachable manager IP")
	flag.Parse()
	if os.Getenv("DATABASE_URL") == "" && os.Getenv("PGHOST") == "" {
		log.Fatal("DATABASE_URL or PostgreSQL environment required")
	}
	pool, err := pgxpool.New(context.Background(), os.Getenv("DATABASE_URL"))
	if err != nil {
		log.Fatal("database configuration invalid")
	}
	defer pool.Close()
	pair, err := tls.LoadX509KeyPair(*caCert, *caKey)
	if err != nil {
		log.Fatal("device CA loading failed")
	}
	ca, err := x509.ParseCertificate(pair.Certificate[0])
	if err != nil {
		log.Fatal("device CA invalid")
	}
	secret, err := os.ReadFile(*pepperFile)
	if err != nil {
		log.Fatal("enrollment secret loading failed")
	}
	signer, ok := pair.PrivateKey.(crypto.Signer)
	if !ok {
		log.Fatal("device CA signing key invalid")
	}
	store, err := manager.NewStore(pool, secret, ca, signer)
	if err != nil {
		log.Fatal("enrollment configuration invalid")
	}
	if store.Migrate(context.Background()) != nil {
		log.Fatal("database migration failed")
	}
	api, err := manager.NewAPI(store, *origin)
	if err != nil {
		log.Fatal(err)
	}
	if *deviceListen != "" {
		endpoint, e := url.Parse(*deviceURL)
		if e != nil || endpoint.Scheme != "wss" || endpoint.Host == "" || endpoint.Path != "/connect" {
			log.Fatal("valid device URL required")
		}
		api.(*manager.API).DeviceURL = *deviceURL
		roots := x509.NewCertPool()
		roots.AddCert(ca)
		deviceServer := &http.Server{Addr: *deviceListen, Handler: http.HandlerFunc(api.(*manager.API).DeviceHandler), ReadHeaderTimeout: 5 * time.Second, MaxHeaderBytes: 16384, TLSConfig: &tls.Config{MinVersion: tls.VersionTLS13, ClientAuth: tls.RequireAndVerifyClientCert, ClientCAs: roots}}
		go func() { log.Fatal(deviceServer.ListenAndServeTLS(*tlsCert, *tlsKey)) }()
	}
	if *mediaListen != "" {
		if *mediaIP == "" || api.(*manager.API).ConfigureMedia(*mediaListen, *mediaIP) != nil {
			log.Fatal("media gateway configuration failed")
		}
	}
	server := &http.Server{Addr: *listen, Handler: api, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 15 * time.Second, WriteTimeout: 15 * time.Second, IdleTimeout: 60 * time.Second, MaxHeaderBytes: 16384, TLSConfig: &tls.Config{MinVersion: tls.VersionTLS13}}
	log.Print("internal manager enrollment API starting; operator authentication is NOT enabled")
	if *webRoot != "" {
		static := http.FileServer(http.Dir(*webRoot))
		u, _ := url.Parse(*origin)
		server.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if strings.HasPrefix(r.URL.Path, "/api/") {
				api.ServeHTTP(w, r)
				return
			}
			if r.TLS == nil || r.Host != u.Host || r.Method != "GET" {
				http.Error(w, "not available", 403)
				return
			}
			w.Header().Set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'")
			w.Header().Set("X-Content-Type-Options", "nosniff")
			w.Header().Set("Referrer-Policy", "no-referrer")
			if r.URL.Path == "/" {
				r.URL.Path = "/manager.html"
				w.Header().Set("Cache-Control", "no-store")
			}
			static.ServeHTTP(w, r)
		})
	}
	log.Fatal(server.ListenAndServeTLS(*tlsCert, *tlsKey))
}
