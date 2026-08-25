import { createClient as createGeneratedBackendClient, SdkworkBackendClient, } from "../generated/server-openapi/src/index";
export { SdkworkBackendClient, createGeneratedBackendClient };
export * from "../generated/server-openapi/src/types";
export * from "../generated/server-openapi/src/api";
export * from "../generated/server-openapi/src/http";
export * from "../generated/server-openapi/src/auth";
export function createClient(config) {
    return createGeneratedBackendClient(config);
}
//# sourceMappingURL=index.js.map