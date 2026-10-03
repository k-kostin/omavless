//go:build !p4_cookie_transport

// SPDX-License-Identifier: MIT
package main

import (
	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/device"
)

func fixtureBind(*observations) conn.Bind         { return conn.NewDefaultBind() }
func fixturePrepare(*device.Device, string) error { return nil }
