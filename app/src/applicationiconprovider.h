#pragma once

#include <QQuickImageProvider>

#include <memory>

[[nodiscard]] std::unique_ptr<QQuickImageProvider>
createApplicationIconProvider();
