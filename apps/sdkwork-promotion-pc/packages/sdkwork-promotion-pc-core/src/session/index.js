// 会话契约：定义 PC 应用会话 token 提供者的类型边界
// 实际 token 读取由 root src/bootstrap 注入（localStorage / appbase IAM runtime）
// 创建会话上下文，默认返回匿名 token 提供者，便于 shell 在未注入时降级渲染
export function createPromotionPcSessionContext(tokenProvider = () => ({})) {
    return {
        tokenProvider,
        isAuthenticated() {
            const tokens = tokenProvider();
            return Boolean(tokens.authToken || tokens.accessToken);
        },
    };
}
//# sourceMappingURL=index.js.map