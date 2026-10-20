// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Operation-specific startup copy, including both nonterminal waiting notices.
use crate::startup::StartupOperation;

/// Complete initial, two-minute and five-minute copy for every startup operation.
pub struct StartupMessages([[&'static str; 3]; 10]);
impl StartupMessages {
    /// Return initial, longer-than-usual and extended-wait messages in that order.
    pub const fn operation(&self, operation: StartupOperation) -> [&'static str; 3] {
        self.0[match operation {
            StartupOperation::Preparing => 0,
            StartupOperation::Dependencies => 1,
            StartupOperation::Runtime => 2,
            StartupOperation::Device => 3,
            StartupOperation::CustomNodes => 4,
            StartupOperation::Database => 5,
            StartupOperation::Server => 6,
            StartupOperation::Frontend => 7,
            StartupOperation::Reveal => 8,
            StartupOperation::Ready => 9,
        }]
    }
    /// Enumerate all translated labels for registry coverage validation.
    pub fn labels(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().flatten().copied()
    }
}

pub(super) static EN: StartupMessages = StartupMessages([
    [
        "Preparing startup…",
        "Preparing startup…\nThis is taking longer than usual.",
        "Preparing startup…\nStill waiting. You can open output for details.",
    ],
    [
        "Preparing Python dependencies…",
        "Preparing Python dependencies…\nThis is taking longer than usual.",
        "Preparing Python dependencies…\nStill waiting. You can open output for details.",
    ],
    [
        "Loading the ComfyUI runtime…",
        "Loading the ComfyUI runtime…\nThis is taking longer than usual.",
        "Loading the ComfyUI runtime…\nStill waiting. You can open output for details.",
    ],
    [
        "Preparing the compute device…",
        "Preparing the compute device…\nThis is taking longer than usual.",
        "Preparing the compute device…\nStill waiting. You can open output for details.",
    ],
    [
        "Loading ComfyUI custom nodes…",
        "Loading ComfyUI custom nodes…\nThis is taking longer than usual.",
        "Loading ComfyUI custom nodes…\nStill waiting. You can open output for details.",
    ],
    [
        "Preparing the ComfyUI database…",
        "Preparing the ComfyUI database…\nThis is taking longer than usual.",
        "Preparing the ComfyUI database…\nStill waiting. You can open output for details.",
    ],
    [
        "Waiting for the ComfyUI server…",
        "Waiting for the ComfyUI server…\nThis is taking longer than usual.",
        "Waiting for the ComfyUI server…\nStill waiting. You can open output for details.",
    ],
    [
        "Loading the workspace…",
        "Loading the workspace…\nThis is taking longer than usual.",
        "Loading the workspace…\nStill waiting. You can open output for details.",
    ],
    [
        "Opening the main window…",
        "Opening the main window…\nThis is taking longer than usual.",
        "Opening the main window…\nStill waiting. You can open output for details.",
    ],
    [
        "Ready",
        "Ready\nThis is taking longer than usual.",
        "Ready\nStill waiting. You can open output for details.",
    ],
]);

pub(super) static ZH_HANS: StartupMessages = StartupMessages([
    [
        "正在准备启动…",
        "正在准备启动…\n所需时间比平时稍长。",
        "正在准备启动…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在准备 Python 依赖项…",
        "正在准备 Python 依赖项…\n所需时间比平时稍长。",
        "正在准备 Python 依赖项…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在加载 ComfyUI 运行环境…",
        "正在加载 ComfyUI 运行环境…\n所需时间比平时稍长。",
        "正在加载 ComfyUI 运行环境…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在准备计算设备…",
        "正在准备计算设备…\n所需时间比平时稍长。",
        "正在准备计算设备…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在加载 ComfyUI 自定义节点…",
        "正在加载 ComfyUI 自定义节点…\n所需时间比平时稍长。",
        "正在加载 ComfyUI 自定义节点…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在准备 ComfyUI 数据库…",
        "正在准备 ComfyUI 数据库…\n所需时间比平时稍长。",
        "正在准备 ComfyUI 数据库…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在等待 ComfyUI 服务器…",
        "正在等待 ComfyUI 服务器…\n所需时间比平时稍长。",
        "正在等待 ComfyUI 服务器…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在加载工作区…",
        "正在加载工作区…\n所需时间比平时稍长。",
        "正在加载工作区…\n仍在等待。可打开输出查看详情。",
    ],
    [
        "正在打开主窗口…",
        "正在打开主窗口…\n所需时间比平时稍长。",
        "正在打开主窗口…\n仍在等待。可打开输出查看详情。",
    ],
    ["已就绪", "已就绪\n所需时间比平时稍长。", "已就绪\n仍在等待。可打开输出查看详情。"],
]);

pub(super) static JA: StartupMessages = StartupMessages([
    [
        "起動を準備中…",
        "起動を準備中…\n通常より時間がかかっています。",
        "起動を準備中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "Python の依存関係を準備中…",
        "Python の依存関係を準備中…\n通常より時間がかかっています。",
        "Python の依存関係を準備中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "ComfyUI ランタイムを読み込み中…",
        "ComfyUI ランタイムを読み込み中…\n通常より時間がかかっています。",
        "ComfyUI ランタイムを読み込み中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "計算デバイスを準備中…",
        "計算デバイスを準備中…\n通常より時間がかかっています。",
        "計算デバイスを準備中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "ComfyUI カスタムノードを読み込み中…",
        "ComfyUI カスタムノードを読み込み中…\n通常より時間がかかっています。",
        "ComfyUI カスタムノードを読み込み中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "ComfyUI データベースを準備中…",
        "ComfyUI データベースを準備中…\n通常より時間がかかっています。",
        "ComfyUI データベースを準備中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "ComfyUI サーバーを待機中…",
        "ComfyUI サーバーを待機中…\n通常より時間がかかっています。",
        "ComfyUI サーバーを待機中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "ワークスペースを読み込み中…",
        "ワークスペースを読み込み中…\n通常より時間がかかっています。",
        "ワークスペースを読み込み中…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "メインウィンドウを開いています…",
        "メインウィンドウを開いています…\n通常より時間がかかっています。",
        "メインウィンドウを開いています…\n引き続き待機中です。出力で詳細を確認できます。",
    ],
    [
        "準備完了",
        "準備完了\n通常より時間がかかっています。",
        "準備完了\n引き続き待機中です。出力で詳細を確認できます。",
    ],
]);

pub(super) static KO: StartupMessages = StartupMessages([
    [
        "시작 준비 중…",
        "시작 준비 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "시작 준비 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "Python 의존성 준비 중…",
        "Python 의존성 준비 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "Python 의존성 준비 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "ComfyUI 런타임 불러오는 중…",
        "ComfyUI 런타임 불러오는 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "ComfyUI 런타임 불러오는 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "연산 장치 준비 중…",
        "연산 장치 준비 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "연산 장치 준비 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "ComfyUI 사용자 정의 노드 불러오는 중…",
        "ComfyUI 사용자 정의 노드 불러오는 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "ComfyUI 사용자 정의 노드 불러오는 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "ComfyUI 데이터베이스 준비 중…",
        "ComfyUI 데이터베이스 준비 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "ComfyUI 데이터베이스 준비 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "ComfyUI 서버 기다리는 중…",
        "ComfyUI 서버 기다리는 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "ComfyUI 서버 기다리는 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "작업 공간 불러오는 중…",
        "작업 공간 불러오는 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "작업 공간 불러오는 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "메인 창 여는 중…",
        "메인 창 여는 중…\n평소보다 시간이 오래 걸리고 있습니다.",
        "메인 창 여는 중…\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
    [
        "준비 완료",
        "준비 완료\n평소보다 시간이 오래 걸리고 있습니다.",
        "준비 완료\n계속 기다리는 중입니다. 출력에서 자세히 볼 수 있습니다.",
    ],
]);

pub(super) static ES: StartupMessages = StartupMessages([
    [
        "Preparando el inicio…",
        "Preparando el inicio…\nEstá tardando más de lo habitual.",
        "Preparando el inicio…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Preparando dependencias de Python…",
        "Preparando dependencias de Python…\nEstá tardando más de lo habitual.",
        "Preparando dependencias de Python…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Cargando el entorno de ComfyUI…",
        "Cargando el entorno de ComfyUI…\nEstá tardando más de lo habitual.",
        "Cargando el entorno de ComfyUI…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Preparando el dispositivo de cálculo…",
        "Preparando el dispositivo de cálculo…\nEstá tardando más de lo habitual.",
        "Preparando el dispositivo de cálculo…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Cargando nodos personalizados de ComfyUI…",
        "Cargando nodos personalizados de ComfyUI…\nEstá tardando más de lo habitual.",
        "Cargando nodos personalizados de ComfyUI…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Preparando la base de datos de ComfyUI…",
        "Preparando la base de datos de ComfyUI…\nEstá tardando más de lo habitual.",
        "Preparando la base de datos de ComfyUI…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Esperando al servidor de ComfyUI…",
        "Esperando al servidor de ComfyUI…\nEstá tardando más de lo habitual.",
        "Esperando al servidor de ComfyUI…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Cargando el espacio de trabajo…",
        "Cargando el espacio de trabajo…\nEstá tardando más de lo habitual.",
        "Cargando el espacio de trabajo…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Abriendo la ventana principal…",
        "Abriendo la ventana principal…\nEstá tardando más de lo habitual.",
        "Abriendo la ventana principal…\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
    [
        "Listo",
        "Listo\nEstá tardando más de lo habitual.",
        "Listo\nSeguimos esperando. Abre la salida para ver detalles.",
    ],
]);
