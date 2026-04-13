document.addEventListener('DOMContentLoaded', () => {
    initRadarNodes();
    initSpeedMeter();
    initSmoothScroll();
    initTerminalAnimation();
    initProductSwitcher();
});

/**
 * Tactical Radar: Genera pequeños nodos (detecciones) aleatorias.
 */
function initRadarNodes() {
    const container = document.getElementById('radar-nodes');
    if (!container) return;

    setInterval(() => {
        const node = document.createElement('div');
        node.className = 'radar-node';
        
        // Posición aleatoria en el círculo
        const angle = Math.random() * Math.PI * 2;
        const radius = Math.random() * 40 + 10; // entre 10% y 50% del radio
        
        node.style.left = `${50 + Math.cos(angle) * radius}%`;
        node.style.top = `${50 + Math.sin(angle) * radius}%`;
        
        // Color aleatorio (Cian o Rojo para alertas)
        if (Math.random() > 0.8) {
            node.classList.add('alert');
        }

        container.appendChild(node);

        // Desvanecer y eliminar
        setTimeout(() => {
            node.style.opacity = '0';
            setTimeout(() => node.remove(), 1000);
        }, 2000);
    }, 400);
}

/**
 * Speed Meter: Fluctuación realista de la velocidad.
 */
function initSpeedMeter() {
    const meter = document.getElementById('speed-meter');
    if (!meter) return;

    setInterval(() => {
        const base = 1.42;
        const jitter = (Math.random() - 0.5) * 0.08;
        const finalValue = (base + jitter).toFixed(2);
        meter.innerText = `${finalValue}M Rows/s`;
    }, 1500);
}

/**
 * Smooth Scroll para navegación interna.
 */
function initSmoothScroll() {
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function (e) {
            e.preventDefault();
            const target = document.querySelector(this.getAttribute('href'));
            if (target) {
                target.scrollIntoView({
                    behavior: 'smooth'
                });
            }
        });
    });
}

// Estilos extra inyectados para los nodos del radar
const style = document.createElement('style');
style.textContent = `
    .radar-node {
        position: absolute;
        width: 4px;
        height: 4px;
        background: var(--accent-color);
        border-radius: 50%;
        box-shadow: 0 0 10px var(--accent-color);
        transition: opacity 1s;
        transform: translate(-50%, -50%);
    }
    .radar-node.alert {
        background: #FF3D00;
        box-shadow: 0 0 10px #FF3D00;
        width: 6px;
        height: 6px;
    }
`;
document.head.appendChild(style);

/**
 * Terminal Animation: Maneja logs específicos por producto.
 */
function initTerminalAnimation() {
    const container = document.getElementById('terminal-content');
    if (!container) return;

    let currentLine = 0;
    let currentLogSet = "extension"; // Default set
    const cursor = container.querySelector('.cursor');

    const logLibrary = {
        extension: [
            { text: "[INFO] Open Data Hunt Core v1.2.1-nitro (Extension Mode)", type: "info" },
            { text: "[INIT] Attaching to VS Code Extension Host...", type: "info" },
            { text: "[DETECT] Large Data Explorer: Active", type: "success" },
            { text: "[INDEX] Mapping forensic evidence (1.4M rows/s)...", type: "info" },
            { text: "[SUCCESS] Virtual table initialized.", type: "success" },
            { text: "[HUNT] Found IOC pattern: 185.12.x.x", type: "alert" }
        ],
        standalone: [
            { text: "[INFO] zen-ioc Platform v1.0.0 (Standalone Mode)", type: "info" },
            { text: "[INIT] Starting Ghost-Analysis Triage Platform...", type: "info" },
            { text: "[HOST] Multi-platform Parity Layer: Windows/Linux", type: "success" },
            { text: "[STATUS] Memory-mapped I/O: Online", type: "info" },
            { text: "[HUNT] Extraction module: Active (Zero-Footprint)", type: "alert" },
            { text: "[INFO] Standalone hunter ready for field operation.", type: "success" }
        ],
        pipeline: [
            { text: "[BOOT] Nitro-Search Cloud Pipeline v0.0.1-dev", type: "info" },
            { text: "[AUTH] Verifying Forensic Certificates...", type: "info" },
            { text: "[WAIT] Awaiting Node Deployment...", type: "alert" },
            { text: "[CORE] Open Data Hunt Core: Pending SDK Linkage", type: "info" }
        ]
    };

    function addLine() {
        const logs = logLibrary[currentLogSet];
        if (currentLine >= logs.length) {
            return; // Stay at last line until switch
        }

        const log = logs[currentLine];
        const lineElement = document.createElement('span');
        lineElement.className = `terminal-line ${log.type}`;
        lineElement.innerText = log.text;
        
        container.insertBefore(lineElement, container.querySelector('.cursor'));
        currentLine++;

        container.scrollTop = container.scrollHeight;
        setTimeout(addLine, Math.random() * 600 + 300);
    }

    // Exposed function to switch logs
    window.switchTerminalLogs = function(type) {
        container.innerHTML = '<span class="cursor">_</span>';
        currentLine = 0;
        currentLogSet = type;
        addLine();
    };

    setTimeout(addLine, 1000);
}

/**
 * ProductSwitcher: Maneja el intercambio de contenido dinámico.
 */
function initProductSwitcher() {
    const buttons = document.querySelectorAll('.selector-btn');
    const blocks = document.querySelectorAll('.content-block');

    buttons.forEach(btn => {
        btn.addEventListener('click', () => {
            const target = btn.getAttribute('data-target');

            // Actualizar botones
            buttons.forEach(b => b.classList.remove('active'));
            btn.classList.add('active');

            // Actualizar bloques de contenido
            blocks.forEach(block => {
                block.classList.remove('active');
                if (block.id === `${target}-info`) {
                    block.classList.add('active');
                }
            });

            // Sincronizar terminal
            if (window.switchTerminalLogs) {
                window.switchTerminalLogs(target);
            }
        });
    });
}
