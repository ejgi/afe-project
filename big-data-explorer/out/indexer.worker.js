"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
const worker_threads_1 = require("worker_threads");
const fs = __importStar(require("fs"));
const { fileName, maxRows } = worker_threads_1.workerData;
const CHUNK_SIZE = 1024 * 1024 * 5; // 5MB buffer
const buffer = Buffer.alloc(CHUNK_SIZE);
let lineOffsets = new Float64Array(maxRows || 1000000);
let lineLengths = new Float32Array(maxRows || 1000000);
let totalFileLines = 0;
try {
    const fileDescriptor = fs.openSync(fileName, 'r');
    const stats = fs.statSync(fileName);
    const fileSize = stats.size;
    let position = 0;
    let lineStart = 0;
    let lastReportTime = Date.now();
    let inQuotes = false; // HYBRID: State Machine Memory
    while (position < fileSize) {
        const bytesRead = fs.readSync(fileDescriptor, buffer, 0, CHUNK_SIZE, position);
        if (bytesRead === 0)
            break;
        for (let i = 0; i < bytesRead; i++) {
            const byte = buffer[i];
            if (byte === 34) { // ASCII 34 is '"' (Double Quote)
                inQuotes = !inQuotes; // HYBRID: Toggle State (Handles escaped "" perfectly by flipping twice)
            }
            else if (byte === 10 && !inQuotes) { // ASCII 10 is '\n' (Newline), only cut if NOT in quotes
                const absolutePos = position + i;
                if (totalFileLines >= lineOffsets.length) {
                    const newOffsets = new Float64Array(lineOffsets.length * 2);
                    newOffsets.set(lineOffsets);
                    lineOffsets = newOffsets;
                    const newLengths = new Float32Array(lineLengths.length * 2);
                    newLengths.set(lineLengths);
                    lineLengths = newLengths;
                }
                lineOffsets[totalFileLines] = lineStart;
                lineLengths[totalFileLines] = absolutePos - lineStart;
                if (lineLengths[totalFileLines] > 0) {
                    const lastCharBuffer = Buffer.alloc(1);
                    fs.readSync(fileDescriptor, lastCharBuffer, 0, 1, absolutePos - 1);
                    if (lastCharBuffer[0] === 13) { // '\r'
                        lineLengths[totalFileLines]--;
                    }
                }
                lineStart = absolutePos + 1;
                totalFileLines++;
            }
        }
        position += bytesRead;
        // Report progress back exactly every 100ms avoiding spam
        const now = Date.now();
        if (now - lastReportTime > 100) {
            worker_threads_1.parentPort?.postMessage({
                type: 'progress',
                percentage: Math.round((position / fileSize) * 100)
            });
            lastReportTime = now;
        }
    }
    if (lineStart < fileSize) {
        lineOffsets[totalFileLines] = lineStart;
        lineLengths[totalFileLines] = fileSize - lineStart;
        totalFileLines++;
    }
    fs.closeSync(fileDescriptor);
    // Send the TypedArrays back zero-copy via Shared Memory Transfer
    worker_threads_1.parentPort?.postMessage({
        type: 'done',
        totalLines: totalFileLines,
        offsets: lineOffsets.buffer,
        lengths: lineLengths.buffer
    }, [lineOffsets.buffer, lineLengths.buffer]);
}
catch (err) {
    worker_threads_1.parentPort?.postMessage({ type: 'error', message: err.message });
}
//# sourceMappingURL=indexer.worker.js.map