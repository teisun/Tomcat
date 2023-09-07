package com.tomcat.websocket;

import cn.hutool.core.util.StrUtil;
import com.tomcat.exceptions.GlobalExceptionHandler;
import com.tomcat.nettyws.annotation.*;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.service.AiCTutor;
import com.tomcat.service.UserService;
import com.tomcat.utils.JwtUtil;
import io.netty.handler.codec.http.*;
import io.netty.handler.timeout.IdleStateEvent;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang.StringUtils;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.core.userdetails.UserDetails;
import org.springframework.security.core.userdetails.UserDetailsService;
import org.springframework.util.MultiValueMap;

import javax.annotation.PreDestroy;
import java.io.IOException;
import java.util.List;
import java.util.Map;

@ServerEndpoint(path = "${ws.websocketPath}",host = "${ws.host}",port = "${ws.port}")
@Slf4j
//@Component
public class NettyWebSocketServer {

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;

    @Value("${jwt.tokenPrefix}")
    private String tokenPrefix;

    @Autowired
    private JwtUtil jwtUtil;

    @Autowired
    private UserDetailsService userDetailsService;

    @Autowired
    private UserService userService;

    @Autowired
    private AiCTutor aiClient;

    @Autowired
    GlobalExceptionHandler globalExceptionHandler;

    String uid;
    MessageProcessor messageProcessor;

    public NettyWebSocketServer(){
        log.info("init NettyWebSocketServer：" + toString());
//        log.info("Thread.currentThread().getId()：" + Thread.currentThread().getId() +
//                " Thread.currentThread().getName():" + Thread.currentThread().getName());
    }


    /**
     *建立ws连接前的配置
     */
    @BeforeHandshake
    public void handshake(Session session, HttpHeaders headers, @RequestParam String req, @RequestParam MultiValueMap reqMap, @PathVariable String arg, @PathVariable Map pathMap){
        log.info("NettyWebSocketServer handshake");
        log.info("reqMap.toString():" + reqMap.toString());
        if(!tokenCheck(reqMap)){
            tokenCheckFail(session);
        }

    }

    private boolean tokenCheck(MultiValueMap reqMap){
        log.info("tokenHeader:" + tokenHeader);
        List reqList = (List) reqMap.get(tokenHeader);
        if (reqList == null || reqList.isEmpty()){
            return false;
        }
        String _jwtToken = (String) reqList.get(0);
        log.info("_jwtToken:" + _jwtToken);
        if (StringUtils.isBlank(_jwtToken) || !_jwtToken.startsWith(this.tokenPrefix)) {
            return false;
        }
        String authToken = _jwtToken.substring(this.tokenPrefix.length());
        String username = jwtUtil.getUserNameFromToken(authToken);
        log.info("checking authentication " + username);
        if (StringUtils.isBlank(username) ) {
            return false;
        }
        UserDetails userDetails = this.userDetailsService.loadUserByUsername(username);
        // 校验token
        if (!jwtUtil.validateToken(authToken, userDetails)) {
            return false;
        }

        uid = jwtUtil.getFieldFromToken(JwtUtil.KEY_USER_ID, authToken);
        if (StrUtil.isBlank(uid)) return false;
        // jwt token 校验通过
        log.info("websocket checking authentication pass!! username:" + username);

        return true;
    }

    private void tokenCheckFail(Session session) {
        // 返回401未授权状态
        log.info("websocket checking authentication fail!!");
        // 设置响应状态为401
        HttpResponse response = new DefaultHttpResponse(HttpVersion.HTTP_1_1, HttpResponseStatus.UNAUTHORIZED);
        // 添加响应头
        response.headers().set("Content-Type", "application/json");

        // 将响应刷新到客户端
        session.sendObj(response);
        session.close();
    }

    @OnOpen
    public void onOpen(Session session, HttpHeaders headers, @RequestParam String req, @RequestParam MultiValueMap reqMap, @PathVariable String arg, @PathVariable Map pathMap){
        log.info("NettyWebSocketServer onOpen");
        log.info("Thread.currentThread().getId()：" + Thread.currentThread().getId() +
                " Thread.currentThread().getName():" + Thread.currentThread().getName());
        log.info("onOpen userId:" + uid);
        session.setAttribute(JwtUtil.KEY_USER_ID, uid);
        messageProcessor = new MessageProcessor(aiClient, session, uid);
        WsSessionManager.add(uid, session);

    }

    @OnClose
    public void onClose(Session session) throws IOException {
        log.info("NettyWebSocketServer onClose");
        WsSessionManager.remove(uid);
    }

    @OnError
    public void onError(Session session, Throwable throwable) {
        log.info("NettyWebSocketServer onError");
        throwable.printStackTrace();
        WsSessionManager.remove(uid);
    }

    @OnMessage
    public void onMessage(Session session, String message) {
        log.info("NettyWebSocketServer onMessage");
        log.info("msg：" + message);
//        session.sendText("Hello Netty!");
        try {
            messageProcessor.processor(message);
        }catch (Exception e){
            globalExceptionHandler.handleUnKnow(e);
        }

    }

    @OnBinary
    public void onBinary(Session session, byte[] bytes) {
        log.info("NettyWebSocketServer onBinary");
        for (byte b : bytes) {
            System.out.println(b);
        }
        session.sendBinary(bytes);
    }

    @OnEvent
    public void onEvent(Session session, Object evt) {
        log.info("NettyWebSocketServer onEvent");
        if (evt instanceof IdleStateEvent) {
            IdleStateEvent idleStateEvent = (IdleStateEvent) evt;
            switch (idleStateEvent.state()) {
                case READER_IDLE:
                    log.info("read idle");
                    break;
                case WRITER_IDLE:
                    log.info("write idle");
                    break;
                case ALL_IDLE:
                    log.info("all idle");
                    break;
                default:
                    break;
            }
        }
    }


    @PreDestroy
    public void beforeDestroy() {
        log.info("NettyWebSocketServer beforeDestroy：" + this.toString());
    }


}
