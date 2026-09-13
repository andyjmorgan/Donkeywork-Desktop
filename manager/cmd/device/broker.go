package main

import (
	"bytes"
	"context"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"time"
)

func brokerRequest(ctx context.Context, socket string, msg bridgewire.Message) bridgewire.Message {
	reply := bridgewire.Message{Version: 1, Type: "response", ID: msg.ID, Status: 503, Body: json.RawMessage(`{"error":"local session broker unavailable"}`)}
	if socket == "" || !bridgewire.Allowed(msg.Method, msg.Path) {
		return reply
	}
	transport := &http.Transport{DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
		return (&net.Dialer{Timeout: 3 * time.Second}).DialContext(ctx, "unix", socket)
	}}
	defer transport.CloseIdleConnections()
	client := &http.Client{Timeout: 12 * time.Second, Transport: transport, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	req, err := http.NewRequestWithContext(ctx, msg.Method, "http://localhost"+msg.Path, bytes.NewReader(msg.Body))
	if err != nil {
		return reply
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Origin", "http://localhost")
	response, err := client.Do(req)
	if err != nil {
		return reply
	}
	defer response.Body.Close()
	data, err := io.ReadAll(io.LimitReader(response.Body, bridgewire.Limit-4096))
	if err != nil || !json.Valid(data) {
		return reply
	}
	reply.Status = response.StatusCode
	reply.Body = data
	return reply
}
