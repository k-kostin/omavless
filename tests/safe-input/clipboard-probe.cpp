// SPDX-License-Identifier: MIT
// Test-only offscreen Qt offer. It never accesses the desktop clipboard.
#include <QGuiApplication>
#include <QClipboard>
#include <QMimeData>
#include <QTimer>
#include <QThread>
#include <QWindow>
#include <cstdio>

namespace {
int requests = 0;
class Offer final : public QMimeData {
public:
    QStringList formats() const override { return {QStringLiteral("text/plain")}; }
    QVariant retrieveData(const QString &, QMetaType) const override {
        ++requests;
        if (qEnvironmentVariable("OMAVLESS_PROBE_KIND") == "slow") QThread::msleep(250);
        return QString(256 * 1024, QLatin1Char('x'));
    }
};
void startup() {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") != "offscreen"
        || qEnvironmentVariable("OMAVLESS_PASSIVE_PROBE") != "1") return;
    QTimer::singleShot(900, qApp, [] {
        // Quickshell also constructs a GUI application in its supervisor.
        // Instrument only the worker which actually owns our fixture window.
        if (QGuiApplication::allWindows().isEmpty()) return;
        auto clipboard = QGuiApplication::clipboard();
        std::fprintf(stderr, "SAFE_CLIPBOARD_PROBE initialized windows=%d\n", int(QGuiApplication::allWindows().size()));
        auto offer = new Offer;
        offer->text();
        std::fprintf(stderr, "SAFE_CLIPBOARD_PROBE source-control=%d\n", requests);
        requests = 0;
        clipboard->setMimeData(offer);
        QTimer::singleShot(500, qApp, [] {
            const bool baseline = qEnvironmentVariable("OMAVLESS_PROBE_BASELINE") == "1";
            const bool pass = baseline ? requests > 0 : requests == 0;
            std::fprintf(stderr, "SAFE_CLIPBOARD_PROBE requests=%d expected=%s result=%s\n",
                         requests, baseline ? "positive" : "zero", pass ? "PASS" : "FAIL");
            QGuiApplication::clipboard()->clear();
            // The owning fixture controller closes QML through its normal IPC
            // finish, not a raw application exit from the preload callback.
        });
    });
}
Q_COREAPP_STARTUP_FUNCTION(startup)
}
