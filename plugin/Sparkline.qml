// SPDX-License-Identifier: MIT
import QtQuick

Canvas {
  id: root

  property var rxValues: []
  property var txValues: []
  property color rxColor: "#8bd5ca"
  property color txColor: "#c6a0f6"
  property color guideColor: "#33ffffff"

  antialiasing: true
  renderStrategy: Canvas.Cooperative

  onRxValuesChanged: requestPaint()
  onTxValuesChanged: requestPaint()
  onRxColorChanged: requestPaint()
  onTxColorChanged: requestPaint()
  onWidthChanged: requestPaint()
  onHeightChanged: requestPaint()

  function drawSeries(ctx, values, color, maximum, dashed) {
    if (!values || values.length < 2 || maximum <= 0) return
    ctx.beginPath()
    ctx.strokeStyle = color
    ctx.lineWidth = 2
    ctx.lineJoin = "round"
    ctx.lineCap = "round"
    ctx.setLineDash(dashed ? [5, 4] : [])
    for (var i = 0; i < values.length; i++) {
      var x = 1 + i * (width - 2) / Math.max(1, values.length - 1)
      var y = height - 2 - Math.max(0, Number(values[i]) || 0) * (height - 4) / maximum
      if (i === 0) ctx.moveTo(x, y)
      else ctx.lineTo(x, y)
    }
    ctx.stroke()
    ctx.setLineDash([])
  }

  onPaint: {
    var ctx = getContext("2d")
    ctx.resetTransform()
    ctx.clearRect(0, 0, width, height)
    ctx.beginPath()
    ctx.strokeStyle = guideColor
    ctx.lineWidth = 1
    ctx.moveTo(0, height - 1)
    ctx.lineTo(width, height - 1)
    ctx.stroke()
    var maximum = 1
    var i
    for (i = 0; i < rxValues.length; i++) maximum = Math.max(maximum, Number(rxValues[i]) || 0)
    for (i = 0; i < txValues.length; i++) maximum = Math.max(maximum, Number(txValues[i]) || 0)
    drawSeries(ctx, rxValues, rxColor, maximum, false)
    // Dashed upload remains visible when RX and TX happen to be identical;
    // its gaps reveal the solid download line underneath instead of hiding it.
    drawSeries(ctx, txValues, txColor, maximum, true)
  }
}
