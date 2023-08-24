package com.tomcat.websocket;

import com.tomcat.utils.JwtUtil;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.handler.codec.http.*;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang.StringUtils;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.core.userdetails.UserDetails;
import org.springframework.security.core.userdetails.UserDetailsService;
import org.springframework.stereotype.Component;

import java.net.URI;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

@Component
@Slf4j
public class JWTDecoder extends ChannelInboundHandlerAdapter {

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;

    @Value("${jwt.tokenPrefix}")
    private String tokenPrefix;

    @Autowired
    JwtUtil jwtUtil;

    @Autowired
    private UserDetailsService userDetailsService;


    @Override
    public void channelRead(ChannelHandlerContext ctx, Object msg) throws Exception {
        // 如果是HttpRequest请求
        if (null != msg && msg instanceof FullHttpRequest) {
            FullHttpRequest req = (FullHttpRequest) msg;
            String uri = req.uri();

            Map urlParams = getUrlParams(uri);
            // 解析Authorization头JWT
            String _jwtToken = (String) urlParams.get(tokenHeader);
            log.info("tokenHeader:" + tokenHeader);
            log.info("_jwtToken:" + _jwtToken);
            if (StringUtils.isBlank(_jwtToken) || !_jwtToken.startsWith(this.tokenPrefix)) {
                tokenCheckFail(ctx);
                return;
            }


            String authToken = _jwtToken.substring(this.tokenPrefix.length());
            String username = jwtUtil.getUserNameFromToken(authToken);
            log.info("checking authentication " + username);
            if (StringUtils.isNotBlank(username) && SecurityContextHolder.getContext().getAuthentication() == null) {
                UserDetails userDetails = this.userDetailsService.loadUserByUsername(username);
                // 校验token
                if (jwtUtil.validateToken(authToken, userDetails)) {
                    // 存储用户信息到ChannelHandlerContext中
                    log.info("websocket checking authentication pass!! username:" + username);

                } else {
                    tokenCheckFail(ctx);
                    return;
                }
            }


            //如果url包含参数，需要处理
            if (uri.contains("?")) {
                String newUri = uri.substring(0, uri.indexOf("?"));
                log.info("newUri:" + newUri);
                req.setUri(newUri);
            }
        }

        // 传递给后续handler
        super.channelRead(ctx, msg);
    }

    private static void tokenCheckFail(ChannelHandlerContext ctx) {
        // 返回401未授权状态
        log.info("websocket checking authentication fail!!");
        // 设置响应状态为401
        HttpResponse response = new DefaultHttpResponse(HttpVersion.HTTP_1_1, HttpResponseStatus.UNAUTHORIZED);
        // 添加响应头
        response.headers().set("Content-Type", "application/json");

        // 将响应刷新到客户端
        ctx.writeAndFlush(response);
        ctx.channel().close();
    }


    private Map getUrlParams(String url) {
        Map<String, String> map = new LinkedHashMap<>();
        try {
            URI uri = new URI("http://example.com" + url);
            String query = uri.getQuery();
            if (null == query) return map;
            String[] pairs = query.split("&");

            for (String pair : pairs) {
                String[] kv = pair.split("=", 2);
                map.put(kv[0], kv[1]);
            }

        } catch (Exception e) {
            throw new RuntimeException(e);
        }

        return Collections.unmodifiableMap(map);
    }

}