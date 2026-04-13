"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CSVDocument = void 0;
class CSVDocument {
    static async create(uri) {
        return new CSVDocument(uri);
    }
    _uri;
    constructor(uri) {
        this._uri = uri;
    }
    get uri() { return this._uri; }
    dispose() {
        // No-op for now. 
    }
}
exports.CSVDocument = CSVDocument;
//# sourceMappingURL=CSVDocument.js.map