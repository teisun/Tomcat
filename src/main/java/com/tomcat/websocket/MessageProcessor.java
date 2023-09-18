package com.tomcat.websocket;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.utils.SpringContextUtils;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

@Slf4j
public class MessageProcessor {

    AiTutorController aiClient;
    Session session;

    String uid;


    public MessageProcessor(Session session, String uid) {
        this.aiClient = SpringContextUtils.getBean(AiTutorController.class);
        this.session = session;
        this.uid = uid;
    }


    private void sendText(ChatResp resp) {
        String jsonStr = JSONUtil.toJsonStr(resp);
        log.info("MessageProcessor sendText:\n" + jsonStr);
        session.sendText(JSONUtil.toJsonStr(resp));
    }


    public void onError(Session session, Throwable t) {
    }

    public void onClose(Session session) {

    }


    private void commandNotFound(String command) {
        ChatResp<TipsResp> resp = new ChatResp<>();
        resp.setCode(404);
        resp.setDescribe("Command " + command + " not found!");
        String jsonStr = JSONUtil.toJsonStr(resp);
        session.sendText(new TextWebSocketFrame(jsonStr));
    }


    public void processor(String msg) throws InvocationTargetException, IllegalAccessException {

        ChatReq chatReq = JSONUtil.toBean(msg, ChatReq.class);
        chatReq.setUid(uid);
        log.info(msg);
        String cmd = chatReq.getCommand();
        Method call = null;
        Method[] methods = aiClient.getClass().getDeclaredMethods();
        for(Method method : methods){
            if(method.isAnnotationPresent(CmdMapping.class)){
                CmdMapping cmdMapping = method.getAnnotation(CmdMapping.class);
                if(StrUtil.equals(cmdMapping.value(), cmd)){
                    call = method;
                    break;
                }
            }
        }
        if(call == null){
            commandNotFound(chatReq.getCommand());
            return;
        }

        ChatResp resp = (ChatResp) call.invoke(aiClient, chatReq);
        sendText(resp);

        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
    }


}
