package com.tomcat.websocket;

import com.tomcat.utils.JwtUtil;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.SimpleChannelInboundHandler;
import io.netty.handler.codec.http.FullHttpRequest;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.net.URI;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

@Slf4j
public class JWTDecoder extends ChannelInboundHandlerAdapter {

  private String tokenHeader;

  private JwtUtil jwtUtil;

  public JWTDecoder(String tokenHeader, JwtUtil jwtUtil) {
    this.tokenHeader = tokenHeader;
    this.jwtUtil = jwtUtil;
  }

  @Override
  public void channelRead(ChannelHandlerContext ctx, Object msg) throws Exception {
    // 如果是HttpRequest请求
    if (null != msg && msg instanceof FullHttpRequest) {
      FullHttpRequest req = (FullHttpRequest) msg;
      String uri = req.uri();

      Map urlParams = getUrlParams(uri);
      // 解析Authorization头JWT
      String jwtToken = (String) urlParams.get(tokenHeader);
      log.info("tokenHeader:" +  tokenHeader);
      log.info("jwtToken:" + jwtToken);

//      Boolean isValid = validateJWT(jwtToken);
//      if (isValid) {
//        // 存储用户信息到ChannelHandlerContext中
//        ctx.attr(USER_INFO).set(parseJWT(jwtToken));
//      } else {
//        // 返回401未授权状态
//        ctx.channel().close();
//        return;
//      }

      //如果url包含参数，需要处理
      if(uri.contains("?")){
        String newUri=uri.substring(0,uri.indexOf("?"));
        log.info("newUri:" + newUri);
        req.setUri(newUri);
      }
    }

    // 传递给后续handler
//    ctx.fireChannelRead(msg);
    super.channelRead(ctx, msg);
  }


  private Map getUrlParams(String url){
    Map<String,String> map = new LinkedHashMap<>();
    try {
      URI uri = new URI("http://example.com" + url);
      String query = uri.getQuery();
      if(null == query) return map;
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