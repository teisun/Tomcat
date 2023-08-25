package com.tomcat.websocket;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.unfbx.chatgpt.OpenAiClient;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import io.netty.channel.Channel;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.core.task.TaskExecutor;
import org.springframework.stereotype.Component;

import java.util.ArrayList;
import java.util.List;

@Component
public class MessageProcessor {

    @Autowired
    private TaskExecutor wsTaskExecutor;

    @Autowired
    private OpenAiClient aiClient;


    public void processor(String msg) {

        wsTaskExecutor.execute(new Runnable() {
            @Override
            public void run() {
                //TODO
                //        检查缓存中是否存在上下文
                //        请求openai API 获得返回数据
                //        推送消息到订阅用户的设备上
                //        异步生成消息摘要用于搜索
                String uid = msg.split(":")[0];
                String message = msg.split(":")[1];
                String messageContext = (String) LocalCache.CACHE.get(uid);
                List<Message> messages = new ArrayList<>();
                if (StrUtil.isNotBlank(messageContext)) {
                    messages = JSONUtil.toList(messageContext, Message.class);
                    if (messages.size() >= 10) {
                        messages = messages.subList(1, 10);
                    }
                    Message currentMessage = Message.builder().content(message).role(Message.Role.USER).build();
                    messages.add(currentMessage);
                } else {
                    Message currentMessage = Message.builder().content(message).role(Message.Role.USER).build();
                    messages.add(currentMessage);
                }
                ChatCompletionResponse response = aiClient.chatCompletion(messages);
                Message responseMag = response.getChoices().get(0).getMessage();
                Channel channel = ChannelHandlerPool.getChannelMap().get(uid);
                channel.writeAndFlush(new TextWebSocketFrame("服务器消息 "+ uid+"："+ JSONUtil.toJsonStr(responseMag)));
                LocalCache.CACHE.put(uid, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
            }
        });
    }



}
